maturin build --release
pip install --force-reinstall (Get-ChildItem target/wheels/athanor_bio_tools-*.whl | Select-Object -Last 1).FullName
