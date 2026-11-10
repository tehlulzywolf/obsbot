#include <stdio.h>
#include <stddef.h>
#include <linux/videodev2.h>
#define P(t) printf(#t " size=%zu\n", sizeof(struct t));
#define F(t, f) printf(#t "." #f " off=%zu\n", offsetof(struct t, f));
int main(void) {
    P(v4l2_capability) F(v4l2_capability, capabilities) F(v4l2_capability, device_caps)
    P(v4l2_format) F(v4l2_format, type)
    P(v4l2_pix_format)
    P(v4l2_requestbuffers) F(v4l2_requestbuffers, memory) F(v4l2_requestbuffers, count) F(v4l2_requestbuffers, type)
    P(v4l2_buffer) F(v4l2_buffer, index) F(v4l2_buffer, type) F(v4l2_buffer, bytesused)
    F(v4l2_buffer, flags) F(v4l2_buffer, field) F(v4l2_buffer, memory) F(v4l2_buffer, m)
    F(v4l2_buffer, length) F(v4l2_buffer, timestamp)
    P(v4l2_streamparm)
    P(v4l2_control)
    printf("S_FMT=%lu REQBUFS=%lu QUERYBUF=%lu QBUF=%lu DQBUF=%lu S_PARM=%lu QUERYCAP=%lu S_CTRL=%lu G_CTRL=%lu\n",
        VIDIOC_S_FMT, VIDIOC_REQBUFS, VIDIOC_QUERYBUF, VIDIOC_QBUF, VIDIOC_DQBUF,
        VIDIOC_S_PARM, VIDIOC_QUERYCAP, VIDIOC_S_CTRL, VIDIOC_G_CTRL);
    return 0;
}
