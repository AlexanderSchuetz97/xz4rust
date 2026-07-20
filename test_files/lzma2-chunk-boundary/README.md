# LZMA2 chunk-boundary fixtures

These files test persistent decoding at complete LZMA2 chunk boundaries.
They are DATA fragments extracted from a VCDIFF secondary-compressor
stream. The tests do not parse VCDIFF.

The producer was xdelta 3.2.0 from the
[official release](https://github.com/jmacd/xdelta/releases/tag/v3.2.0).
xdelta is licensed under Apache-2.0.
The producer configuration reports `SECONDARY_LZMA=1`,
`EXTERNAL_COMPRESSION=0`, and `XD3_WIN32=1`.

Generation commands:

```text
python generate_fixture.py
xdelta3.exe -e -a -A -N -S lzma -W 16384 -s source.bin target.bin id2.vcdiff
xdelta3.exe -e -a -A -N -S none -W 16384 -s source.bin target.bin none.vcdiff
xdelta3.exe printhdrs id2.vcdiff
xdelta3.exe -d -a -s source.bin id2.vcdiff decoded.bin
```

Source generation starts with the ASCII seed
`vcdiff-id2-feasibility-source`. The seed is repeatedly replaced by
its 32-byte SHA-256 digest. Those digests are concatenated and
truncated to form the 131,072-byte `source.bin`.

`target.bin` has six purpose-generated 16,384-byte text sections.
For window `w` and rows `r` starting at zero, it appends:

```text
window={w:02}; row={r:05}; class=ID2-FEASIBILITY; token={token}; value={value:05};\n
```

`token` is the first 12 hex characters of SHA-256 over
`owned-row-{w}-{r}`. `value` is `(w * 4099 + r * 17) % 100000`.
Each section is truncated to 16,384 bytes, then all six are joined.
The inputs contain no third-party content and are released for
redistribution with this repository's tests.

SHA-256 values:

| Item | Size | SHA-256 |
| --- | ---: | --- |
| xdelta archive | 184,374 | `af8ef036cb077a48df080c9a8ac1be4a6e7511c32d11f8bec89b6803a9e52576` |
| xdelta executable | 336,896 | `53d90226615f217d3380c39892833311b4e24acd863e1ca01f14b5e772e2e6d0` |
| `generate_fixture.py` | 1,233 | `e71fdc5a7d294d6022a6830f7dffcc7b6c7578f76846e8b9113489aa13b098ea` |
| source input | 131,072 | `4ec1e45ab24b6c8ff8ac75692496692cc5908a67492c2d6497310268e69e70ee` |
| target output | 98,304 | `c21ff467100a57e3495cf97bd025a9c903c32a85fd927f5d13b559d2b197daae` |
| decoded output | 98,304 | `c21ff467100a57e3495cf97bd025a9c903c32a85fd927f5d13b559d2b197daae` |
| ID-2 VCDIFF | 16,714 | `f40d8e39994dfd7460cf63883764159cd4fae8285d3cc8c4f8ef231a969f007c` |
| uncompressed VCDIFF | 98,435 | `5e12b575e3ea2c78e85eb03e68f3f154dbcaecf2cdef136c55396128cfb7da61` |

Extracted DATA fragments:

| File | VCDIFF offset | Size | SHA-256 |
| --- | ---: | ---: | --- |
| `fragment-0.bin` | 24 | 2,841 | `36fe64f941f1efdfaa261d4e7481fb3d2c1083c0c09cdbb4fdc0fa93ca0b5f36` |
| `fragment-1.bin` | 2,887 | 2,737 | `cb368b8d2d193ede8c0433c5141453cb7c2d93a7c7951ee02da8d0760e8b7eb6` |
| `fragment-2.bin` | 5,646 | 2,743 | `a8977b06ce5c1504577ec95c1cab4de6c0c4aef453c8b76bdd387aba56337301` |
| `fragment-3.bin` | 8,411 | 2,728 | `8ffdd45eebf2efb3edb191a9400ba4a6be863d7e2bad5d98de6e5c73680211b8` |
| `fragment-4.bin` | 11,161 | 2,744 | `99110e1bd8d02fda4021323f207a45364b7087dadfa5f6fb06e6da129807b3f0` |
| `fragment-5.bin` | 13,927 | 2,783 | `a1d6f5e34dbffa8e5fefa4e21b1fbe110f1a6ad7ee50277ca1f11a790b38618c` |

`expected-output.bin` is the verified `target.bin`. Each 16,384-byte
slice was checked byte-for-byte against the corresponding decoded DATA
section before import.
