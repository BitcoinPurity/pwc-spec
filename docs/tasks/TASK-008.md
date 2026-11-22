# TASK-008: psbt.sign and message signing

Milestone: M7

## Goal

Implement the review before signing, signing after approval, and BIP-322 simple and legacy message signatures.

## Tests written first

- Revoking the grant during pending signing gives `failed` with `PERMISSION_REVOKED`
- A watch-only account asked to sign gives `UNSUPPORTED_OPERATION`
- Reusing an idempotency key with another PSBT
- Inconsistent previous-output values give `PSBT_VALIDATION_FAILED`
- A high fee rate or a nondefault sighash gives `POLICY_VIOLATION`
- Partial signing does not finalize
- Only input `[0]` is signed when requested
- Empty, duplicated or out-of-range `input_indexes`
- An input the account cannot sign
- Inputs already signed
- Net outflow above the limit, without a prompt
- A negative net outflow does not trigger the limit
- Inputs from another network
- A message with non-ASCII characters and trailing whitespace verifies after signing
- The official BIP-322 vectors
- Verifying on an unsupported network
- An exact retry after revocation returns the Operation without `result`
- End to end: create, sign, finalize and broadcast, with the balance following

## Implementation

- The signing review
- Backend signing
- The approval tasks
- `src/message.rs`

## Acceptance criteria

- These tests pass

## Test identifiers covered

C-03, C-04, C-06 to C-10, C-32, C-41 to C-43, C-45, C-59 to C-61, C-63
