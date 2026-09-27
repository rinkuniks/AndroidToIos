cmake_minimum_required(VERSION 3.22.3)

# JNI library that links against the Rust core (built via cargo-ndk or cargo apk).
# The Rust core is compiled separately (see core/build.rs or external build script).
# This CMakeLists simply packages the prebuilt .so into the APK.

project("wabridge_jni")

set(WABRIDGE_JNI_SRC_DIR ${CMAKE_CURRENT_SOURCE_DIR})

# Declare a default shared library; the actual .so files come from the Rust build
# via cargo-ndk or gradle cargo plugin. Here we just define the interface.
add_library(
    wabridge_jni
    SHARED
    IMPORTED
)

# The path to the Rust-built .so is passed via -DWABRIDGE_CORE_DIR at build time.
# In a real build, use cargo-ndk to produce the native libs in jniLibs/.
# See: https://github.com/buridav/cargo-ndk

# Set the imported location based on ABI
# This would be filled by the actual Rust build output:
# set_target_properties(wabridge_jni PROPERTIES
#     IMPORTED_LOCATION ${WABRIDGE_JNI_SRC_DIR}/../jniLibs/${ANDROID_ABI}/libwabridge_jni.so
# )
