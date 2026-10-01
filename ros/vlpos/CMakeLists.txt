cmake_minimum_required(VERSION 3.16)
project(vlpos)

find_package(rosidl_default_generators REQUIRED)
find_package(std_msgs REQUIRED)
find_package(sensor_msgs REQUIRED)
find_package(geometry_msgs REQUIRED)

rosidl_generate_interfaces(${PROJECT_NAME}
  "msg/H2oPointCloud.msg"
  "msg/H2oPoint.msg"
  "msg/H2oPointf.msg"
  "msg/CompressedPointCloud2.msg"
  "msg/Pose6D.msg"
  "msg/SbgImuData.msg"
  "msg/SbgImuStatus.msg"
  DEPENDENCIES std_msgs sensor_msgs geometry_msgs
)

ament_package()
