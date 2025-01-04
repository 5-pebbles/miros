pub fn round_up_to_boundary(address: usize, boundary: usize) -> usize {
    boundary * (address / boundary)
}
