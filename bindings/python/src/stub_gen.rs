use acquiesce_py::stub_info;
use pyo3_stub_gen::Result;

fn main() -> Result<()> {
    stub_info()?.generate()
}
