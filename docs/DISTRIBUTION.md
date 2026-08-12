# Distribution compliance

Dantalian includes `fdk-aac-rust`, a **Third-Party Modified Version of the
Fraunhofer FDK AAC Codec Library for Android**. This project is not an official
Fraunhofer project and is not endorsed by Fraunhofer. Every distributor is
responsible for reading and complying with the complete license in
[`NOTICE`](../NOTICE). This document is operational guidance and is not a
replacement for that license.

## Source distributions

Every source distribution must include, without alteration:

- [`NOTICE`](../NOTICE), containing the complete FDK AAC software license;
- [`README.md`](../README.md), containing the dated prominent modification
  notice, modified-version name, summary of changes, and patent warning;
- [`MODULE_LICENSE_FRAUNHOFER`](../MODULE_LICENSE_FRAUNHOFER), retained as
  licensing metadata.

The source distribution must also include the Rust codec source and all
Dantalian modifications that are distributed with the application. Do not
represent the modified codec as an official Fraunhofer release.

## Build and source availability

`fdk-aac-rust` is used without its optional FFI feature, so a C/C++ compiler is
not required for this project's pure Rust codec path. Its build still reads
reference tables from a pinned upstream source tree. The first build therefore
requires GitHub/network access, or a compatible local source tree supplied with
`FDK_AAC_SOURCE_DIR`.

For reproducible releases, retain the resolved `fdk-aac-rust` version in
`Cargo.lock` and record the exact corresponding upstream revision used by the
crate build.

## Binary distributions

If Dantalian is distributed as a binary, package, container, or hosted service
whose distribution includes the AAC codec, provide all of the following to each
recipient where applicable:

1. the complete `NOTICE` text in the accompanying documentation or materials;
2. a free-of-charge copy of the complete corresponding source code for
   `fdk-aac-rust` and all distributed modifications, using an offer and delivery
   method that recipients can actually access;
3. the prominent modified-version name and dated change notice;
4. no use of the Fraunhofer name to endorse or promote the modified version;
5. no copyright license fee charged for use, copying, or distribution of the
   codec or its modifications.

Record the exact `fdk-aac-rust` version and the corresponding source revision
for each binary release. A link to a moving branch is not an adequate record of
corresponding source.

## Patent rights

The FDK AAC software license grants no express or implied patent license.
Distribution or use of an AAC encoder or decoder may require authorization from
applicable patent owners or a licensing administrator. Copyright-license
compliance does not establish patent clearance for any product, territory, or
use case.

## Review checklist

Before publishing a release that includes AAC support, confirm:

- `NOTICE` is present and unchanged;
- `MODULE_LICENSE_FRAUNHOFER` is present;
- the README notice includes the modification date and modified-version name;
- the corresponding Rust codec source is available to recipients;
- the exact dependency version and source revision are recorded;
- no product documentation implies Fraunhofer endorsement;
- patent clearance has been reviewed for the intended distribution and use.
