pub(crate) trait AbiTransfer: Sized {
    type Abi;

    unsafe fn into_abi(self) -> Self::Abi;
    unsafe fn from_abi(abi: Self::Abi) -> Self;
    unsafe fn clone_from_abi(abi: Self::Abi) -> Self;
}
