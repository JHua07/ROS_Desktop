# Minimal SBG IMU data message (IMU-only build)
std_msgs/Header header

# Time since sensor is powered up [us]
uint32 time_stamp

# IMU Status
SbgImuStatus imu_status

# Filtered Accelerometer [m/s^2]
geometry_msgs/Vector3 accel

# Filtered Gyroscope [rad/s]
geometry_msgs/Vector3 gyro
