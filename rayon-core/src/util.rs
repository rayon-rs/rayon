pub(super) fn leak<T>(v: T) -> &'static T {
    Box::leak(Box::new(v))
}
