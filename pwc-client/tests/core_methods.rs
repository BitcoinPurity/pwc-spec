//! TASK-014: one typed function per Core method. The fake Provider checks every request against the schema.

mod common;

use common::fake_provider::{Fake, Reply};
use common::fixtures::{self as fx, ADDRESS, PSBT, TXID};
use pwc_client::*;
use serde_json::{json, Value};

fn answer(method: &str, _: &Value) -> Reply {
    Reply::Ok(match method {
        "pwc.getCapabilities" => fx::capabilities(),
        "session.getInfo" => fx::session_info(),
        "session.requestPermissions" => fx::operation(method, "awaiting_approval", None),
        "session.getPermissions" => json!({"grants": [fx::grant()]}),
        "session.revokePermissions" => json!({"revoked": true}),
        "wallet.list" => json!({"wallets": [fx::wallet()]}),
        "account.list" => json!({"accounts": [fx::account()]}),
        "account.getBalance" => fx::balance(),
        "address.getReceiveAddress" | "address.create" => fx::address(0),
        "address.list" => json!({"addresses": [fx::address(0)], "next_cursor": "c-2"}),
        "utxo.list" => json!({"utxos": [fx::utxo(0)]}),
        "transaction.list" => json!({"transactions": [fx::transaction()]}),
        "transaction.get" => json!({"transaction": fx::transaction()}),
        "psbt.create" => fx::operation(method, "completed", Some(fx::create_result())),
        "psbt.releaseReservation" => json!({"released": false}),
        "psbt.sign" => fx::operation(method, "awaiting_approval", None),
        "psbt.combine" => json!({"psbt": PSBT, "psbt_version": 0}),
        "psbt.finalize" => fx::finalize_result(),
        "transaction.broadcast" => fx::operation(method, "completed", Some(json!({"txid": TXID}))),
        "message.sign" => fx::operation(method, "completed", Some(fx::message_result())),
        "message.verify" => json!({"valid": true}),
        "operation.get" => fx::operation("psbt.sign", "completed", Some(fx::sign_result())),
        "operation.cancel" => fx::operation("psbt.sign", "canceled", None),
        "local.unpair" => json!({"unpaired": true}),
        _ => return Reply::Err("UNSUPPORTED_METHOD"),
    })
}

/// The answers above satisfy the strict schema, so the Client is tested against conforming data.
#[test]
fn answers_conform() {
    let file = format!(
        "{}/../pwc-schema/pwc-core-0.2.1.schema.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let schema: Value = serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
    let methods = schema["x-pwc-methods"].as_object().unwrap();
    assert_eq!(methods.len(), 24);
    for method in methods.keys() {
        let Reply::Ok(result) = answer(method, &json!({})) else {
            panic!("no answer for {method}")
        };
        common::schema::core(&format!("{method}.result"), &result).unwrap();
        if let Some(inner) = result.get("result").filter(|_| result.get("operation_id").is_some()) {
            let op_method = result["method"].as_str().unwrap();
            common::schema::core(&format!("{op_method}.operationResult"), inner).unwrap();
        }
    }
}

#[tokio::test]
async fn discovery_session_and_grants() {
    let fake = Fake::start(answer);
    let c = fake.client().await;

    let caps = c.get_capabilities().await.unwrap();
    assert_eq!(caps.pwc_version, "0.2.1");
    assert_eq!(caps.provider.platform, Platform::Desktop);
    assert_eq!(caps.script_types, [ScriptType::P2wpkh, ScriptType::P2shP2wpkh]);
    assert_eq!(
        caps.message_formats,
        [MessageFormat::Bip322Simple, MessageFormat::Legacy]
    );
    assert_eq!(caps.limits.high_fee_rate, 500_000);
    assert_eq!(fake.params("pwc.getCapabilities"), json!({}));

    assert_eq!(c.get_session_info().await.unwrap().grant_ids, ["g-1"]);

    let op = c
        .request_permissions(RequestPermissionsParams {
            scopes: vec!["wallets:read".into(), "psbt:sign".into()],
            network: Some("regtest".into()),
            wallet_ids: None,
            account_ids: Some(vec!["a-1".into()]),
            expires_at: Some(fx::T1.into()),
            constraints: Some(GrantConstraints {
                max_amount_sats_per_request: Some(50_000),
                valid_until: None,
            }),
            idempotency_key: "k-1".into(),
        })
        .await
        .unwrap();
    assert_eq!(op.state, OperationState::AwaitingApproval);
    assert_eq!(op.expires_at.as_deref(), Some(fx::T1));
    assert_eq!(
        fake.params("session.requestPermissions"),
        json!({"scopes": ["wallets:read", "psbt:sign"], "network": "regtest", "account_ids": ["a-1"],
               "expires_at": fx::T1, "constraints": {"max_amount_sats_per_request": "50000"},
               "idempotency_key": "k-1"})
    );

    let grants = c.get_permissions().await.unwrap();
    assert_eq!(grants[0].status, GrantStatus::Active);
    assert_eq!(
        grants[0].constraints.as_ref().unwrap().max_amount_sats_per_request,
        Some(50_000)
    );

    assert!(c.revoke_permissions("g-1", "k-2").await.unwrap());
    assert_eq!(
        fake.params("session.revokePermissions"),
        json!({"grant_id": "g-1", "idempotency_key": "k-2"})
    );
}

#[tokio::test]
async fn wallets_accounts_and_reads() {
    let fake = Fake::start(answer);
    let c = fake.client().await;

    let wallets = c.list_wallets().await.unwrap();
    assert_eq!(
        (wallets[0].wallet_kind, wallets[0].state),
        (WalletKind::Software, WalletState::Ready)
    );
    let accounts = c.list_accounts("w-1").await.unwrap();
    assert!(accounts[0].can_sign);
    assert_eq!(fake.params("account.list"), json!({"wallet_id": "w-1"}));

    let balance = c.get_balance("a-1").await.unwrap();
    assert_eq!(balance.spendable_sats, 600_000);
    assert_eq!(
        balance.sync,
        SyncStatus {
            state: SyncState::Synced,
            tip_height: Some(1000)
        }
    );

    assert_eq!(c.get_receive_address("a-1").await.unwrap().role, AddressRole::Receive);
    assert_eq!(fake.params("address.getReceiveAddress"), json!({"account_id": "a-1"}));
    c.create_address("a-1", Some("Invoice 7".into()), "k-3").await.unwrap();
    assert_eq!(
        fake.params("address.create"),
        json!({"account_id": "a-1", "label": "Invoice 7", "idempotency_key": "k-3"})
    );

    let page = c.list_addresses("a-1", Some("c-1".into()), Some(10)).await.unwrap();
    assert_eq!(page.next_cursor.as_deref(), Some("c-2"));
    assert_eq!(
        fake.params("address.list"),
        json!({"account_id": "a-1", "cursor": "c-1", "page_size": 10})
    );

    let utxos = c.list_utxos("a-1", None, None).await.unwrap();
    assert_eq!(utxos.utxos[0].value_sats, 500_000);
    assert_eq!(utxos.next_cursor, None);
    assert_eq!(fake.params("utxo.list"), json!({"account_id": "a-1"}));

    let txs = c.list_transactions("a-1", None, Some(5)).await.unwrap();
    assert_eq!(txs.transactions[0].status, TransactionStatus::Confirmed);
    let tx = c.get_transaction("a-1", TXID).await.unwrap();
    assert_eq!((tx.net_change_sats, tx.fee_sats), (-301_000, Some(1000)));
    assert_eq!(
        fake.params("transaction.get"),
        json!({"account_id": "a-1", "txid": TXID})
    );
}

#[tokio::test]
async fn psbt_lifecycle() {
    let fake = Fake::start(answer);
    let c = fake.client().await;

    let op = c
        .create_psbt(CreatePsbtParams {
            account_id: "a-1".into(),
            outputs: vec![Payment {
                address: ADDRESS.into(),
                amount_sats: 300_000,
            }],
            fee_policy: FeePolicy::Rate { sat_per_kvb: 1250 },
            selected_utxos: Some(vec![OutPoint {
                txid: TXID.into(),
                vout: 1,
            }]),
            replaceable: Some(true),
            idempotency_key: "k-4".into(),
        })
        .await
        .unwrap();
    let created = op.create_psbt_result().unwrap();
    assert_eq!((created.fee_sats, created.vsize), (313, 141));
    assert_eq!(created.reservation_id.as_deref(), Some("r-1"));
    assert_eq!(
        fake.params("psbt.create"),
        json!({"account_id": "a-1", "outputs": [{"address": ADDRESS, "amount_sats": "300000"}],
               "fee_policy": {"type": "rate", "sat_per_kvb": "1250"},
               "selected_utxos": [{"txid": TXID, "vout": 1}], "replaceable": true, "idempotency_key": "k-4"})
    );

    assert!(!c.release_reservation("a-1", "r-1", "k-5").await.unwrap());
    assert_eq!(
        fake.params("psbt.releaseReservation"),
        json!({"account_id": "a-1", "reservation_id": "r-1", "idempotency_key": "k-5"})
    );

    let params = SignPsbtParams {
        account_id: "a-1".into(),
        psbt: PSBT.into(),
        input_indexes: Some(vec![0]),
        idempotency_key: "k-6".into(),
    };
    let op = c.sign_psbt(params).await.unwrap();
    assert_eq!(op.sign_psbt_result(), Err(Error::OperationPending));
    assert_eq!(
        fake.params("psbt.sign"),
        json!({"account_id": "a-1", "psbt": PSBT, "input_indexes": [0], "idempotency_key": "k-6"})
    );

    let signed = c
        .get_operation(&op.operation_id)
        .await
        .unwrap()
        .sign_psbt_result()
        .unwrap();
    assert_eq!(signed.signing_status, SigningStatus::Complete);
    assert_eq!(fake.params("operation.get"), json!({"operation_id": "op-1"}));

    assert_eq!(
        c.combine_psbts(vec![PSBT.into(), PSBT.into()])
            .await
            .unwrap()
            .psbt_version,
        0
    );
    assert_eq!(fake.params("psbt.combine"), json!({"psbts": [PSBT, PSBT]}));

    let done = c.finalize_psbt(PSBT).await.unwrap();
    assert!(done.complete);
    assert_eq!(done.txid.as_deref(), Some(TXID));

    let op = c.broadcast_transaction("a-1", "0200", "k-7").await.unwrap();
    assert_eq!(op.broadcast_result().unwrap(), TXID);
    assert_eq!(
        fake.params("transaction.broadcast"),
        json!({"account_id": "a-1", "raw_transaction": "0200", "idempotency_key": "k-7"})
    );

    let canceled = c.cancel_operation("op-1", "k-8").await.unwrap();
    assert_eq!(canceled.state, OperationState::Canceled);
    match canceled.sign_psbt_result() {
        Err(Error::Pwc(e)) => assert_eq!(e.code, ErrorCode::OperationCanceled),
        other => panic!("{other:?}"),
    }
    assert_eq!(
        fake.params("operation.cancel"),
        json!({"operation_id": "op-1", "idempotency_key": "k-8"})
    );
}

#[tokio::test]
async fn wallet_estimate_fee_policy() {
    let fake = Fake::start(answer);
    let c = fake.client().await;
    c.create_psbt(CreatePsbtParams {
        account_id: "a-1".into(),
        outputs: vec![Payment {
            address: ADDRESS.into(),
            amount_sats: 1,
        }],
        fee_policy: FeePolicy::WalletEstimate { target_blocks: 6 },
        selected_utxos: None,
        replaceable: None,
        idempotency_key: "k-9".into(),
    })
    .await
    .unwrap();
    assert_eq!(
        fake.params("psbt.create")["fee_policy"],
        json!({"type": "wallet_estimate", "target_blocks": 6})
    );
}

#[tokio::test]
async fn messages() {
    let fake = Fake::start(answer);
    let c = fake.client().await;
    let op = c
        .sign_message(SignMessageParams {
            account_id: "a-1".into(),
            address: ADDRESS.into(),
            message: "Hello\n".into(),
            format: MessageFormat::Bip322Simple,
            idempotency_key: "k-10".into(),
        })
        .await
        .unwrap();
    let signed = op.sign_message_result().unwrap();
    assert_eq!(signed.format, MessageFormat::Bip322Simple);
    assert_eq!(
        fake.params("message.sign"),
        json!({"account_id": "a-1", "address": ADDRESS, "message": "Hello\n", "format": "bip322-simple",
               "idempotency_key": "k-10"})
    );

    let valid = c
        .verify_message(VerifyMessageParams {
            network: "regtest".into(),
            address: ADDRESS.into(),
            message: "Hello\n".into(),
            signature: signed.signature,
            format: MessageFormat::Legacy,
        })
        .await
        .unwrap();
    assert!(valid);
    assert_eq!(fake.params("message.verify")["format"], "legacy");
}
