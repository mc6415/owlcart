# Changes made for Owlcart

This is librqbit 9.1.0 (https://github.com/ikatson/rqbit, Apache-2.0, see
LICENSE), copied from crates.io with one change:

- `src/read_buf.rs`: the per-peer read buffer (`BUFLEN`) is raised from 32 KiB
  to 256 KiB. A peer's opening "bitfield" message holds one bit per piece, and
  Minerva's multi-terabyte console torrents have around 500,000 pieces, so the
  message is ~60 KiB. With a 32 KiB buffer, every peer was disconnected as soon
  as it sent it, and nothing downloaded.

Remove this copy (and the `[patch.crates-io]` entry in src-tauri/Cargo.toml)
once upstream librqbit handles larger messages.
