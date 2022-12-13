# bier-rust

This repository is an updated version of the [bier-socket-api project](https://github.com/louisna/bier-socket-api). 

It provides an open-source implementation of the Bit Index Explicit Replication multicast forwarding mechanism using the standard socket API.

Moreover, it exposes a socket-like API to discuss with BIER, i.e., send and receive multicast packets from the BIER daemon.

## Differences with bier-socket-api

This repository proposes an implementation in the Rust programming language, which is both safer and cleaner (in my humble opinion), without loosing performance.

Additionally, this project exposes the BIER processing as a library, independently of the the I/O. This is similar to [Cloudflare quiche](https://github.com/cloudflare/quiche). The user must handle the I/O and send the payload to the BIER processing.

Finally, this updated implementation provides tests for every part of the BIER processing, as well as for the BIER configuration binary.

## Limitations compared to bier-socket-api

Currently, this work support communication with a single application/upper layer protocol. It is not possible to register new applications on-the-fly similarly to bier-socketa-api.

## Communicate with BIER

The communication with the BIER daemon is different from bier-socket-api. The C implementation uses QCBOR to send and receive the payloads and the BIER context. In this project, we simply use a packet buffer with varints. The API is exposed in [api.rs](src/api.rs).

## Examples and BIER daemon.

The [main.rs](src/main.rs) file implements a BIER node, forwarding BIER packets.

The [sender.rs](examples/sender.rs) and [receiver.rs](examples/receiver.rs) files show examples of how upper-layer applications/protocols can communicate with the BIER daemon.

## BIER-TE

This implementation currently does not support BIER-TE. This is a future work.

## Cite this work

Even if this implementation is not completely related to bier-socket-api, it heavily relies on the first implementation. Moreover, I did both implementations, and suggest using this one if you don't mind the lack of modularity and BIER-TE compared to bier-socket-api. So, to cite this work:

```
@inproceedings{navarre2022experimenting,
  title={Experimenting with bit index explicit replication},
  author={Navarre, Louis and Rybowski, Nicolas and Bonaventure, Olivier},
  booktitle={Proceedings of the 3rd International CoNEXT Student Workshop},
  pages={17--19},
  year={2022}
}
```

