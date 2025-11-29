cmake_minimum_required(VERSION 3.15)
project(moq_ffi_c_tests C CXX)

set(CMAKE_C_STANDARD 11)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

# Find the moq_ffi library
# Expect it to be built in ../target/release or ../target/debug
if(NOT DEFINED MOQFFI_LIB_DIR)
    set(MOQFFI_LIB_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../target/release")
endif()

if(NOT DEFINED MOQFFI_INCLUDE_DIR)
    set(MOQFFI_INCLUDE_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../include")
endif()

# Platform-specific library name
if(WIN32)
    set(MOQFFI_LIB_NAME "moq_ffi.dll.lib")
    set(MOQFFI_DLL_NAME "moq_ffi.dll")
elseif(APPLE)
    set(MOQFFI_LIB_NAME "libmoq_ffi.dylib")
else()
    set(MOQFFI_LIB_NAME "libmoq_ffi.so")
endif()

# Include directories
include_directories(
    ${MOQFFI_INCLUDE_DIR}
    ${CMAKE_CURRENT_SOURCE_DIR}/include
)

# Link directories
link_directories(${MOQFFI_LIB_DIR})

# Common test utilities
add_library(test_common STATIC
    src/test_framework.c
)
target_include_directories(test_common PUBLIC include)

# Platform-specific libraries
if(UNIX AND NOT APPLE)
    set(PLATFORM_LIBS pthread dl m)
elseif(APPLE)
    set(PLATFORM_LIBS pthread)
elseif(WIN32)
    set(PLATFORM_LIBS ws2_32 userenv bcrypt ntdll)
endif()

# Macro to add a test executable
macro(add_moq_test test_name source_file)
    add_executable(${test_name} ${source_file})
    target_link_libraries(${test_name} test_common moq_ffi ${PLATFORM_LIBS})

    # Add test to CTest
    add_test(NAME ${test_name} COMMAND ${test_name})

    # Set runtime path for finding the shared library
    if(UNIX)
        set_target_properties(${test_name} PROPERTIES
            BUILD_RPATH "${MOQFFI_LIB_DIR}"
            INSTALL_RPATH "${MOQFFI_LIB_DIR}"
        )
    endif()
endmacro()

# Unit Tests
add_moq_test(test_initialization src/test_initialization.c)
add_moq_test(test_lifecycle src/test_lifecycle.c)
add_moq_test(test_connection src/test_connection.c)
add_moq_test(test_publishing src/test_publishing.c)
add_moq_test(test_subscribing src/test_subscribing.c)
add_moq_test(test_track_discovery src/test_track_discovery.c)
add_moq_test(test_error_handling src/test_error_handling.c)
add_moq_test(test_memory_safety src/test_memory_safety.c)

# Integration Tests (C++)
add_executable(test_pubsub_integration src/test_pubsub_integration.cpp)
target_link_libraries(test_pubsub_integration test_common moq_ffi ${PLATFORM_LIBS})
add_test(NAME test_pubsub_integration COMMAND test_pubsub_integration)

add_executable(test_catalog_integration src/test_catalog_integration.cpp)
target_link_libraries(test_catalog_integration test_common moq_ffi ${PLATFORM_LIBS})
add_test(NAME test_catalog_integration COMMAND test_catalog_integration)

add_executable(test_multi_client_integration src/test_multi_client_integration.cpp)
target_link_libraries(test_multi_client_integration test_common moq_ffi ${PLATFORM_LIBS})
add_test(NAME test_multi_client_integration COMMAND test_multi_client_integration)

# Set runtime path for C++ tests
if(UNIX)
    set_target_properties(
        test_pubsub_integration
        test_catalog_integration
        test_multi_client_integration
        PROPERTIES
        BUILD_RPATH "${MOQFFI_LIB_DIR}"
        INSTALL_RPATH "${MOQFFI_LIB_DIR}"
    )
endif()

# Enable testing
enable_testing()

# Print configuration
message(STATUS "MoQ FFI Library Directory: ${MOQFFI_LIB_DIR}")
message(STATUS "MoQ FFI Include Directory: ${MOQFFI_INCLUDE_DIR}")
message(STATUS "MoQ FFI Library Name: ${MOQFFI_LIB_NAME}")
