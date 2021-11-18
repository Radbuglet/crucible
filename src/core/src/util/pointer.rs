pub unsafe fn extend_ref<'src, 'target, T: ?Sized>(obj: &'src T) -> &'target T {
	&*(obj as *const T)
}

pub unsafe fn extend_mut<'src, 'target, T: ?Sized>(obj: &'src mut T) -> &'target mut T {
	&mut *(obj as *mut T)
}
