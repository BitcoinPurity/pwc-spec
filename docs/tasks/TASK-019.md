# TASK-019: Java interface without bitcoinj

Milestone: M18

## Goal

Replace the bitcoinj types of the Java interface by Java types, byte arrays and strings.

## Tests written first

- Every existing Java test, rewritten for `OutPoint`, `TxOut`, `byte[]` and hex strings, passes
- The jar depends on no other artifact

## Implementation

- Records `OutPoint`, `TxOut`; JNI conversions

## Acceptance criteria

- `input/requirements.md` §6.4
