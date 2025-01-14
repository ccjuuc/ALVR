use crate::command;
use alvr_filesystem as afs;
use std::{error::Error, fs};

pub fn choco_install(package: &str) -> Result<(), Box<dyn Error>> {
    command::run_without_shell(
        "powershell",
        &[
            "Start-Process",
            "choco",
            "-ArgumentList",
            &format!("\"install {package} -y\""),
            "-Verb",
            "runAs",
        ],
    )
}

pub fn prepare_x264_windows() {
    const VERSION: &str = "0.164";
    const REVISION: usize = 3086;

    command::download_and_extract_zip(
        &format!(
            "{}/{VERSION}.r{REVISION}/libx264_{VERSION}.r{REVISION}_msvc16.zip",
            "https://github.com/ShiftMediaProject/x264/releases/download",
        ),
        &afs::deps_dir().join("windows/x264"),
    );

    fs::write(
        afs::deps_dir().join("x264.pc"),
        format!(
            r#"
prefix={}
exec_prefix=${{prefix}}/bin/x64
libdir=${{prefix}}/lib/x64
includedir=${{prefix}}/include

Name: x264
Description: x264 library
Version: {VERSION}
Libs: -L${{libdir}} -lx264
Cflags: -I${{includedir}}
"#,
            afs::deps_dir()
                .join("windows/x264")
                .to_string_lossy()
                .replace('\\', "/")
        ),
    )
    .unwrap();

    command::run_without_shell(
        "setx",
        &["PKG_CONFIG_PATH", &afs::deps_dir().to_string_lossy()],
    )
    .unwrap();
}

pub fn prepare_ffmpeg_windows() {
    let download_path = afs::deps_dir().join("windows");
    command::download_and_extract_zip(
        &format!(
            "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/{}",
            "ffmpeg-n5.0-latest-win64-gpl-shared-5.0.zip"
        ),
        &download_path,
    );

    fs::rename(
        download_path.join("ffmpeg-n5.0-latest-win64-gpl-shared-5.0"),
        download_path.join("ffmpeg"),
    )
    .unwrap();
}

pub fn prepare_windows_deps(skip_admin_priv: bool) {
    if !skip_admin_priv {
        choco_install("llvm vulkan-sdk wixtoolset pkgconfiglite").unwrap();
    }

    prepare_x264_windows();
    prepare_ffmpeg_windows();
}

pub fn build_ffmpeg_linux(nvenc_flag: bool) {
    let download_path = afs::deps_dir().join("linux");
    command::download_and_extract_zip(
        "https://codeload.github.com/FFmpeg/FFmpeg/zip/n4.4",
        &download_path,
    );

    let final_path = download_path.join("ffmpeg");

    fs::rename(download_path.join("FFmpeg-n4.4"), &final_path).unwrap();

    command::run_as_bash_in(
        &final_path,
        &format!(
            "./configure {} {} {} {} {} {} {} {} {} {} {} {} {} {} {}",
            "--enable-gpl --enable-version3",
            "--disable-static --enable-shared",
            "--disable-programs",
            "--disable-doc",
            "--disable-avdevice --disable-avformat --disable-swresample --disable-postproc",
            "--disable-network",
            "--enable-lto",
            "--disable-everything",
            /*
               Describing Nvidia specific options --nvccflags:
               nvcc from CUDA toolkit version 11.0 or higher does not support compiling for 'compute_30' (default in ffmpeg)
               52 is the minimum required for the current CUDA 11 version (Quadro M6000 , GeForce 900, GTX-970, GTX-980, GTX Titan X)
               https://arnon.dk/matching-sm-architectures-arch-and-gencode-for-various-nvidia-cards/
               Anyway below 50 arch card don't support nvenc encoding hevc https://developer.nvidia.com/nvidia-video-codec-sdk (Supported devices)
               Nvidia docs:
               https://docs.nvidia.com/video-technologies/video-codec-sdk/ffmpeg-with-nvidia-gpu/#commonly-faced-issues-and-tips-to-resolve-them
            */
            (if nvenc_flag {
                let cuda = pkg_config::Config::new().probe("cuda").unwrap();
                let include_flags = cuda
                    .include_paths
                    .iter()
                    .map(|path| format!("-I{path:?}"))
                    .reduce(|a, b| format!("{a}{b}"))
                    .expect("pkg-config cuda entry to have include-paths");
                let link_flags = cuda
                    .link_paths
                    .iter()
                    .map(|path| format!("-L{path:?}"))
                    .reduce(|a, b| format!("{a}{b}"))
                    .expect("pkg-config cuda entry to have link-paths");

                format!(
                    "{} {} {} --extra-cflags=\"{}\" --extra-ldflags=\"{}\" {}",
                    "--enable-encoder=h264_nvenc --enable-encoder=hevc_nvenc --enable-nonfree",
                    "--enable-cuda-nvcc --enable-libnpp",
                    "--nvccflags=\"-gencode arch=compute_52,code=sm_52 -O2\"",
                    include_flags,
                    link_flags,
                    "--enable-hwaccel=h264_nvenc --enable-hwaccel=hevc_nvenc",
                )
            } else {
                "".to_string()
            }),
            "--enable-encoder=h264_vaapi --enable-encoder=hevc_vaapi",
            "--enable-encoder=libx264 --enable-encoder=libx264rgb --enable-encoder=libx265",
            "--enable-hwaccel=h264_vaapi --enable-hwaccel=hevc_vaapi",
            "--enable-filter=scale --enable-filter=scale_vaapi",
            "--enable-libx264 --enable-libx265 --enable-vulkan",
            "--enable-libdrm",
        ),
    )
    .unwrap();
    command::run_as_bash_in(&final_path, "make -j$(nproc)").unwrap();
}

fn get_oculus_openxr_mobile_loader() {
    let temp_sdk_dir = afs::build_dir().join("temp_download");

    // OpenXR SDK v1.0.18. todo: upgrade when new version is available
    command::download_and_extract_zip(
        "https://securecdn.oculus.com/binaries/download/?id=4421717764533443",
        &temp_sdk_dir,
    );

    let destination_dir = afs::deps_dir().join("android/oculus_openxr/arm64-v8a");
    fs::create_dir_all(&destination_dir).unwrap();

    fs::copy(
        temp_sdk_dir.join("OpenXR/Libs/Android/arm64-v8a/Release/libopenxr_loader.so"),
        destination_dir.join("libopenxr_loader.so"),
    )
    .unwrap();

    fs::remove_dir_all(temp_sdk_dir).ok();
}

pub fn build_android_deps(skip_admin_priv: bool) {
    if cfg!(windows) && !skip_admin_priv {
        choco_install("llvm").unwrap();
    }

    command::run("rustup target add aarch64-linux-android").unwrap();
    command::run("cargo install cargo-apk").unwrap();

    get_oculus_openxr_mobile_loader();
}
