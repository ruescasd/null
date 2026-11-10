cmake_minimum_required(VERSION 3.20)
project(navbench_detour CXX)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# recastnavigation as shipped (main, 9f4ce64, 2026-02-27); RECAST_DIR its
# checkout.
set(RECASTNAVIGATION_DEMO OFF CACHE BOOL "" FORCE)
set(RECASTNAVIGATION_TESTS OFF CACHE BOOL "" FORCE)
set(RECASTNAVIGATION_EXAMPLES OFF CACHE BOOL "" FORCE)
set(RECASTNAVIGATION_DT_POLYREF64 ON CACHE BOOL "" FORCE)
add_subdirectory(${RECAST_DIR} recast)

add_executable(navbench main.cpp)
target_link_libraries(navbench Recast Detour DetourCrowd)
