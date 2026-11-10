#include <stdio.h>
#include <string.h>
#include <fcntl.h>
#include <unistd.h>
#include <sys/ioctl.h>
#include <errno.h>
#include <linux/videodev2.h>

int main(void) {
    printf("STREAMON=%lu STREAMOFF=%lu\n", (unsigned long)VIDIOC_STREAMON,
           (unsigned long)VIDIOC_STREAMOFF);
    for (int n = 0; n < 4; n++) {
        char path[64];
        snprintf(path, sizeof path, "/dev/video%d", n);
        int fd = open(path, O_RDWR);
        if (fd < 0) { printf("%s: open fail\n", path); continue; }
        struct v4l2_capability cap;
        if (ioctl(fd, VIDIOC_QUERYCAP, &cap) == 0) {
            printf("%s: card='%s' driver='%s' caps=%08x device_caps=%08x\n",
                   path, cap.card, cap.driver, cap.capabilities, cap.device_caps);
        } else printf("%s: QUERYCAP errno=%d\n", path, errno);
        struct v4l2_format f;
        memset(&f, 0, sizeof f);
        f.type = V4L2_BUF_TYPE_VIDEO_CAPTURE;
        f.fmt.pix.width = 1280; f.fmt.pix.height = 720;
        f.fmt.pix.pixelformat = V4L2_PIX_FMT_MJPEG;
        f.fmt.pix.field = V4L2_FIELD_NONE;
        int r = ioctl(fd, VIDIOC_S_FMT, &f);
        printf("   S_FMT -> r=%d errno=%d (%s) got %ux%u fourcc=%c%c%c%c\n",
               r, errno, r ? strerror(errno) : "ok",
               f.fmt.pix.width, f.fmt.pix.height,
               f.fmt.pix.pixelformat & 0xff, (f.fmt.pix.pixelformat >> 8) & 0xff,
               (f.fmt.pix.pixelformat >> 16) & 0xff, (f.fmt.pix.pixelformat >> 24) & 0xff);
        close(fd);
    }
    return 0;
}
