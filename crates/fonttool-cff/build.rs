fn main() {
    let mut c_build = cc::Build::new();
    c_build.include("native/src");
    c_build.files([
        "native/src/byte_io.c",
        "native/src/file_io.c",
        "native/src/sfnt_font.c",
        "native/src/sfnt_reader.c",
        "native/src/sfnt_writer.c",
        "native/src/glyf_codec.c",
    ]);
    c_build.compile("fonttool_cff_native_c");

    let mut cpp_build = cc::Build::new();
    cpp_build.cpp(true).std("c++17").include("native/src");
    cpp_build.flag_if_supported("-Wno-unused-parameter").files([
        "native/src/parallel_runtime.cc",
        "native/src/cu2qu.cc",
        "native/src/tt_rebuilder.cc",
    ]);
    cpp_build.compile("fonttool_cff_native_cpp");
}
