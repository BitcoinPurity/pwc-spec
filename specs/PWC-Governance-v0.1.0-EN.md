# Purity Wallet Connect (PWC)
## Governance, Licensing and Change Process

**Specification:** PWC-GOVERNANCE-001  
**Version:** 0.1.0 (review draft)  
**Date:** 2026-11-21  
**Language:** English  
**License:** CC BY 4.0  
**Status:** Proposed; subject to adoption by the Specification Owner

> **Scope rule:** This document states under which license the PWC specifications are published, who decides on changes, how a change is proposed and versioned, how names are requested, how a document moves from draft to stable, how security problems are reported, and what a conformance claim means.

## 1. Scope

This document applies to every document of the PWC family listed in PWC-OVERVIEW-001 and to the companion files distributed with them: schemas, registry, test vectors and test scripts.

## 2. License

### 2.1 Documents and data

The PWC specifications, guides and registry, and the companion data files (the JSON schemas, the registry JSON and the test vectors), are licensed under the **Creative Commons Attribution 4.0 International License (CC BY 4.0)**. Anyone may share and adapt them for any purpose, including commercially, provided they give appropriate credit, link to the license and indicate whether changes were made.

Attribution SHOULD take this form:

> "⟨Document title⟩ ⟨version⟩, Purity Wallet Connect, licensed under CC BY 4.0 (https://creativecommons.org/licenses/by/4.0/)." followed by a note of any changes.

### 2.2 Code

Creative Commons recommends against using its licenses for software. The executable scripts of the test kit and of the schema tools, and the implementations distributed with the specifications, are therefore licensed under the **MIT License**, whose text is in the license file at the root of the repository.

### 2.3 Trademarks and patents

The license covers copyright only. It grants no trademark rights and contains no patent license (Appendix A, G-04).

## 3. Roles

| Role | Responsibility |
|---|---|
| **Specification Owner** | The Bitcoin Purity project, acting through its maintainers. Approves every normative change and every status change, appoints Editors |
| **Editors** | Maintain the documents, classify and merge changes, keep the family consistent, run the registry |
| **Contributors** | Anyone who proposes a change or reports a problem |
| **Implementers** | Build Providers and Clients and publish conformance claims |

## 4. Changes and versioning

### 4.1 Proposing a change

A change is proposed in writing to the Editors, through the canonical repository (Appendix A, G-03). The proposal states the problem, the affected sections and the proposed text.

### 4.2 Classifying changes

| Class | Examples | Version effect (Core §11) | Approval |
|---|---|---|---|
| Editorial | Wording, typos, examples, references | PATCH | Editors |
| Compatible | A new optional method, feature, field or error code | MINOR | Specification Owner |
| Incompatible | Removing or changing the meaning of anything defined | MAJOR; any revision before 1.0 | Specification Owner |

Every document of the family is versioned independently with these rules. Each document names the versions of the other documents it requires in its header.

### 4.3 Review

A normative change is open for review for at least 14 days before approval while the affected document is a draft, and at least 30 days once it is stable. The Specification Owner may shorten review for a security fix (§7). Every approved change is recorded in the change history appendix of the document it changes.

### 4.4 Errata

A confirmed error in a stable document is published as an erratum, and corrected in the next PATCH or MINOR revision.

## 5. Registry requests

PWC-REGISTRY-001 §1 lists who allocates each kind of name. The following procedures apply:

- **`provider_id`:** a publisher requests an identifier from the Editors, giving the desired identifier, the product name, the publisher and a contact. Identifiers are assigned first come, first served. The Editors refuse an identifier that is confusingly similar to an existing one, or that names a product of another publisher.
- **Core and transport-profile names:** allocated only through a revision of the defining specification (§4).
- **Extension names** `vendor.<reverse-domain>.<name>`: allocated by the owner of the domain, without registration (Core §11).
- **External registrations**, such as the Nostr event kind of PWC-TRANSPORT-REMOTE-001: submitted by the Editors to the external registry before the affected document becomes a Candidate.

## 6. Document status

| Status | Meaning | Entry criteria |
|---|---|---|
| Draft | Work in progress | — |
| Review draft | Complete enough for public review; may still change incompatibly | Editors judge it complete |
| Candidate | Expected to become stable unchanged except for fixes | Security audit done and its findings resolved or accepted; external registrations submitted |
| Stable | Deployed standard; incompatible changes require a new MAJOR version | For each transport profile, at least one Provider and one Client from different code bases with valid conformance claims (PWC-CONFORMANCE-001) |
| Superseded | Replaced by a newer version | The newer version is stable |
| Withdrawn | No longer recommended | Decision of the Specification Owner |

All documents of this family are currently review drafts.

## 7. Security reports

- A suspected vulnerability in a specification is reported privately to the security contact (Appendix A, G-02), not in public issues.
- The Editors acknowledge a report within 7 days.
- Disclosure is coordinated: the report stays private until a fix is published or 90 days have passed, whichever is first, unless the reporter agrees otherwise.
- A security fix may be approved with shortened review (§4.3). It is recorded with credit to the reporter if the reporter wishes.
- Vulnerabilities in an implementation are reported to its publisher, not under this process.

## 8. Conformance claims

- A conformance claim is a self-assessment made by the implementer under PWC-CONFORMANCE-001 §1. There is no certification body in this version.
- The Specification Owner MAY keep a public list of published claims and their reports.
- A claim without a report, or with a `fail` outcome, is invalid. The Specification Owner MAY ask an implementer to correct or withdraw an invalid claim, and MAY remove it from the public list.

## 9. Publication

- The normative text of every document is English. Translations are informative, and the English text prevails.
- File names follow the pattern `PWC-<Name>-v<version>-EN.md`; companion files carry the version of the document they belong to.
- Each release of a document is published together with the companion files of the same version.

## Appendix A. Open questions

| ID | Subject | Current draft policy |
|---|---|---|
| G-01 | License of the test-kit scripts | Resolved: MIT (§2.2) |
| G-02 | Security contact | To be named by the Specification Owner |
| G-03 | Canonical repository and permanent schema URIs | Not assigned; schemas use the placeholder domain `purity-wallet-connect.invalid` |
| G-04 | Patent policy | None |
| G-05 | How the Specification Owner records its decisions | To be defined by the Bitcoin Purity maintainers |

## References

- Creative Commons Attribution 4.0 International: https://creativecommons.org/licenses/by/4.0/
- Creative Commons FAQ, on software: https://creativecommons.org/faq/
- PWC-CORE-001 v0.2.1 §11; PWC-REGISTRY-001, PWC-CONFORMANCE-001 and PWC-OVERVIEW-001 v0.1.0

---

**End of PWC-GOVERNANCE-001 v0.1.0.**
