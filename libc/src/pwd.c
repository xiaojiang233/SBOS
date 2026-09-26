#include <errno.h>
#include <pwd.h>
#include <string.h>

static char root_name[] = "root";
static char guest_name[] = "guest";
static char empty_password[] = "";
static char root_home[] = "/Users/Root";
static char guest_home[] = "/Users/Guest";
static char shell_path[] = "/Applications/bash";
static struct passwd accounts[] = {
    { root_name, empty_password, 0, 0, root_name, root_home, shell_path },
    { guest_name, empty_password, 1000, 1000, guest_name, guest_home, shell_path },
};
static size_t account_cursor;

struct passwd *getpwuid(uid_t uid) {
    size_t index;
    for (index = 0; index < sizeof(accounts) / sizeof(accounts[0]); ++index) {
        if (accounts[index].pw_uid == uid) { errno = 0; return &accounts[index]; }
    }
    errno = ENOENT;
    return 0;
}

struct passwd *getpwnam(const char *name) {
    size_t index;
    if (name == 0) { errno = EINVAL; return 0; }
    for (index = 0; index < sizeof(accounts) / sizeof(accounts[0]); ++index) {
        if (strcmp(accounts[index].pw_name, name) == 0) { errno = 0; return &accounts[index]; }
    }
    errno = ENOENT;
    return 0;
}

struct passwd *getpwent(void) {
    if (account_cursor >= sizeof(accounts) / sizeof(accounts[0])) return 0;
    return &accounts[account_cursor++];
}

void setpwent(void) { account_cursor = 0; }
void endpwent(void) { account_cursor = 0; }
