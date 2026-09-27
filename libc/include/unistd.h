#ifndef _SBOS_UNISTD_H
#define _SBOS_UNISTD_H
#include <sys/types.h>

#ifndef _POSIX_VERSION
#define _POSIX_VERSION 200809L
#endif

#define STDIN_FILENO 0
#define STDOUT_FILENO 1
#define STDERR_FILENO 2

ssize_t read(int fd, void *buffer, size_t length);
ssize_t write(int fd, const void *buffer, size_t length);
int close(int fd);
int fsync(int fd);
int fdatasync(int fd);
int unlink(const char *path);
int link(const char *existing_path, const char *new_path);
int rmdir(const char *path);
int getgroups(int size, gid_t list[]);
int access(const char *path, int mode);
int gethostname(char *name, size_t length);
int getdtablesize(void);
int getpagesize(void);
int dup(int fd);
int dup2(int source, int destination);
off_t lseek(int fd, off_t offset, int whence);
pid_t getpid(void);
pid_t getppid(void);
uid_t getuid(void);
uid_t geteuid(void);
gid_t getgid(void);
gid_t getegid(void);
int setuid(uid_t uid);
int setgid(gid_t gid);
pid_t fork(void);
int execve(const char *path, char *const argv[], char *const envp[]);
int execvp(const char *file, char *const argv[]);
int chdir(const char *path);
char *getcwd(char *buffer, size_t size);
int isatty(int fd);
int pipe(int descriptors[2]);
char *ttyname(int fd);
unsigned int sleep(unsigned int seconds);
int usleep(useconds_t microseconds);
unsigned int alarm(unsigned int seconds);
int pause(void);
void _exit(int status) __attribute__((noreturn));
extern char **environ;

#endif
