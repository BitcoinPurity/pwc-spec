# TASK-021: Four-platform jar, publishing and CI

Milestone: M20

## Goal

Build and test on four platforms and publish one jar.

## Tests written first

- CI builds and tests on linux-x86_64, linux-aarch64, macos-aarch64 and windows-x86_64 (no Intel Mac build agent, so not macos-x86_64)
- The assembled jar contains the four libraries

## Implementation

- `-PnativeLibs` in the Gradle build, `maven-publish` to GitHub Packages
- CI jobs: Rust on Windows, Java SDK matrix, jar assembly, publishing on tags

## Acceptance criteria

- `input/requirements.md` §6.4
