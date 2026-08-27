#no_main
use libfuzzer_sys::fuzz_target;
use earnproof_shared::{IssuerStatus, ProofStatus};

fuzz_target!| d: &[u8] | {
    if d.len() > 1024 { return; }
    let _ = IssuerStatus::from_xdr(d);
    let _ = ProofStatus::from_xdr(d);
});
