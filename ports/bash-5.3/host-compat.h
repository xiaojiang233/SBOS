/* Compatibility declarations for Bash's host-side generator programs.
 *
 * mksyntax, mkbuiltins, bashversion and psize are compiled with the build
 * machine's compiler so they can run during the build. Their sources are
 * written against POSIX headers but are compiled with the SBOS config.h, so a
 * few declarations that the SBOS C library provides are missing from the MinGW
 * headers. This header is force-included with -include for host tools only;
 * it never affects code built for SBOS.
 */
#ifndef SBOS_BASH_HOST_COMPAT_H
#define SBOS_BASH_HOST_COMPAT_H

/* Bash's makefile passes its identity macros to the target build through
   SYSTEM_FLAGS, which cannot carry quotes on this host (see tools/bash-cc.sh).
   The target driver therefore defines them itself, and the host-only objects
   (buildversion.o comes from version.c and uses MACHTYPE) get the same values
   from here. They describe the SBOS target, not the machine doing the build. */
#ifndef CONF_HOSTTYPE
#  define CONF_HOSTTYPE "x86_64"
#endif
#ifndef CONF_OSTYPE
#  define CONF_OSTYPE "none"
#endif
#ifndef CONF_MACHTYPE
#  define CONF_MACHTYPE "x86_64-unknown-none"
#endif
#ifndef CONF_VENDOR
#  define CONF_VENDOR "unknown"
#endif
#ifndef PROGRAM
#  define PROGRAM "bash"
#endif

#if defined (_WIN32)

#  include <stdint.h>	/* intmax_t, uintmax_t */
#  include <signal.h>	/* sigset_t */
#  include <sys/types.h>	/* pid_t, off_t, ... */
typedef _sigset_t sigset_t;

/* The MinGW headers have no group or user identifiers at all; Bash's general.h
   declares group_member() in terms of gid_t. Host generators never use the
   value, so a plain 32-bit type is enough to get them compiled. */
#  if !defined (_GID_T_DEFINED) && !defined (__gid_t_defined)
typedef unsigned int gid_t;
#    define _GID_T_DEFINED 1
#  endif

#  ifndef SIGPIPE
#    define SIGPIPE 13
#  endif

#endif /* _WIN32 */

#endif /* SBOS_BASH_HOST_COMPAT_H */
