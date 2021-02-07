# adieu
This is a bytecode parser for older AVG32-based visual novels.

Most of the information about the bytecode format was salvaged from an archived copy of [Waffle](https://github.com/ruin0x11/waffle_osx), an AVG32 engine for Mac OS X.

## Usage

```
cargo run -- unpack SEEN.TXT
cargo run -- disasm SEEN001.TXT
cargo run -- asm SEEN001.adieu
```
