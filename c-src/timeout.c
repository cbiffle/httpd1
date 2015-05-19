#include <sys/select.h>
#include <errno.h>
#include <stdlib.h>

/*
 * Waits for data to become available on a file descriptor within a specified
 * number of seconds.
 */
int wait_for_data(int fd, time_t seconds) {
  struct timeval tv = {
    .tv_sec = seconds,
    .tv_usec = 0,
  };

  fd_set fds;
  FD_ZERO(&fds);
  FD_SET(fd, &fds);

  if (select(fd + 1, &fds, NULL, NULL, &tv) == -1) return -1;
  if (!FD_ISSET(fd, &fds)) {
    errno = ETIMEDOUT;
    return -1;
  }

  return 0;
}

