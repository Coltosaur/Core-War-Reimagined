// Re-embed migrations when they change (#77).
//
// Both the backend's test setup and the `core-war-migrate` binary embed the
// migration set at compile time via `sqlx::migrate!("./migrations")`. Making
// the dependency explicit here guarantees a changed or newly added migration
// is rebuilt into the release artifact rather than served stale from a cached
// build — the migrator the deploy step runs must match the SQL on disk.
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
