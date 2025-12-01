use crate::signature_matches_libc;

#[no_mangle]
unsafe extern "C" fn strlen(start_character: *mut u8) -> usize {
    signature_matches_libc!(strlen(start_character));
    (0_usize..)
        .find(|i| *start_character.add(*i) == 0)
        .unwrap_unchecked()
}
