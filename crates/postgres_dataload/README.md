# Running local dataload test

In a new window navigate to the crate you want to test and run `cargo lambda watch`

Generally use WSL for rust development to avoid (my) ARM64 Windows compilation issues.

```bash
wsl bash -c "source ~/.cargo/env && cd /mnt/c/Users/rober/GitHub/module_2/crates/[crate_name] && cargo build"
```