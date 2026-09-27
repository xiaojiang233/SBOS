#include <errno.h>
#include <grp.h>
#include <string.h>
#include <unistd.h>

int getgroups(int size, gid_t list[]) {
    if (size < 0) { errno = EINVAL; return -1; }
    if (size == 0) return 1;
    if (list == 0) { errno = EFAULT; return -1; }
    if (size < 1) { errno = EINVAL; return -1; }
    list[0] = getgid();
    return 1;
}

static struct group current_group;
static char *no_members[1];
static unsigned int group_cursor;

struct group *getgrgid(gid_t gid) {
    if (gid != 0 && gid != 1000) return 0;
    current_group.gr_name = gid == 0 ? "root" : "users";
    current_group.gr_passwd = "x";
    current_group.gr_gid = gid;
    current_group.gr_mem = no_members;
    return &current_group;
}

struct group *getgrnam(const char *name) {
    if (name == 0) { errno = EFAULT; return 0; }
    if (strcmp(name, "root") == 0) return getgrgid(0);
    if (strcmp(name, "users") == 0) return getgrgid(1000);
    return 0;
}

struct group *getgrent(void) {
    if (group_cursor >= 2) return 0;
    return getgrgid(group_cursor++ == 0 ? 0 : 1000);
}

void setgrent(void) { group_cursor = 0; }
void endgrent(void) { group_cursor = 0; }
