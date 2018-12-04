if [[ "$TRAVIS_RUST_VERSION" == "nightly" ]]
then
    # Don't cache any build output for nightly.
    rm -rf target
else
    rm -rf target/{debug,release}/maildir-pack*
    rm -rf target/{debug,release}/maildir_pack-*
    rm -rf target/{debug,release}/{build,.fingerprint}/maildir-pack-*
    rm -rf target/{debug,release}/deps/maildir_pack-*
    rm -rf target/debug/incremental
fi
