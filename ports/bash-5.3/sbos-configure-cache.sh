#!/bin/sh
# Autoconf cache values that let GNU Bash 5.3 configure finish for the
# freestanding SBOS target. Bash's configure executes its probe programs, and
# SBOS cannot run host-built probe binaries, so every result that a cross
# build should observe is pinned here instead of being detected.
#
# Source this before running configure:
#
#   . ports/bash-5.3/sbos-configure-cache.sh
#
# CC and CPP are deliberately not pinned; the build driver supplies them.

ac_cv_build='x86_64-pc-linux-gnu'
ac_cv_build_alias='x86_64-pc-linux-gnu'
ac_cv_c_bigendian='no'
ac_cv_c_char_unsigned='no'
ac_cv_c_compiler_gnu='yes'
ac_cv_c_const='yes'
ac_cv_c_inline='inline'
ac_cv_c_long_double='yes'
ac_cv_c_stringize='yes'
ac_cv_decl_sys_siglist='no'
ac_cv_func___setostype='no'
ac_cv_func__doprnt='no'
ac_cv_func_alarm='yes'
ac_cv_func_alloca_works='yes'
ac_cv_func_asprintf='no'
ac_cv_func_bcopy='yes'
ac_cv_func_bindtextdomain='no'
ac_cv_func_bzero='no'
ac_cv_func_confstr='no'
ac_cv_func_dlclose='no'
ac_cv_func_dlopen='no'
ac_cv_func_dlsym='no'
ac_cv_func_dup2='yes'
ac_cv_func_fnmatch='no'
ac_cv_func_getaddrinfo='no'
ac_cv_func_getcwd='yes'
ac_cv_func_getdtablesize='yes'
ac_cv_func_getgroups='no'
ac_cv_func_gethostbyname='no'
ac_cv_func_gethostname='yes'
ac_cv_func_getpagesize='no'
ac_cv_func_getpeername='no'
ac_cv_func_getpgrp_void='yes'
ac_cv_func_getrlimit='no'
ac_cv_func_getrusage='no'
ac_cv_func_getservbyname='no'
ac_cv_func_getservent='no'
ac_cv_func_gettext='no'
ac_cv_func_gettimeofday='yes'
ac_cv_func_inet_aton='no'
ac_cv_func_isascii='no'
ac_cv_func_isblank='no'
ac_cv_func_isgraph='no'
ac_cv_func_isinf='no'
ac_cv_func_isprint='no'
ac_cv_func_isspace='no'
ac_cv_func_isxdigit='no'
ac_cv_func_killpg='no'
ac_cv_func_lstat='yes'
ac_cv_func_mbsrtowcs='no'
ac_cv_func_memmove='yes'
ac_cv_func_memset='yes'
ac_cv_func_mkfifo='no'
ac_cv_func_pathconf='no'
ac_cv_func_putenv='no'
ac_cv_func_readlink='no'
ac_cv_func_rename='yes'
ac_cv_func_sbrk='no'
ac_cv_func_select='no'
ac_cv_func_setdtablesize='no'
ac_cv_func_setenv='no'
ac_cv_func_setlinebuf='no'
ac_cv_func_setlocale='yes'
ac_cv_func_setvbuf='no'
ac_cv_func_setvbuf_reversed='no'
ac_cv_func_siginterrupt='no'
ac_cv_func_snprintf='yes'
ac_cv_func_strcasecmp='yes'
ac_cv_func_strchr='yes'
ac_cv_func_strcoll_works='no'
ac_cv_func_strerror='yes'
ac_cv_func_strftime='yes'
ac_cv_func_strnlen='yes'
ac_cv_func_strpbrk='yes'
ac_cv_func_strstr='yes'
ac_cv_func_strdup='yes'
ac_cv_func_strtod='no'
ac_cv_func_strtoimax='no'
ac_cv_func_strtol='yes'
ac_cv_func_strtoll='no'
ac_cv_func_strtoul='yes'
ac_cv_func_strtoull='no'
ac_cv_func_strtoumax='no'
ac_cv_func_sysconf='no'
ac_cv_func_tcgetattr='yes'
ac_cv_func_tcgetpgrp='no'
ac_cv_func_textdomain='no'
ac_cv_func_times='yes'
ac_cv_func_ttyname='yes'
ac_cv_func_tzset='no'
ac_cv_func_ulimit='no'
ac_cv_func_uname='no'
ac_cv_func_unsetenv='no'
ac_cv_func_vasprintf='no'
ac_cv_func_vprintf='yes'
ac_cv_func_vsnprintf='yes'
ac_cv_func_wait3='no'
ac_cv_func_waitpid='yes'
ac_cv_func_wcwidth='no'
ac_cv_func_working_mktime='no'
ac_cv_have_decl_confstr='no'
ac_cv_have_decl_printf='yes'
ac_cv_have_decl_sbrk='no'
ac_cv_have_decl_strcpy='yes'
ac_cv_have_decl_strsignal='no'
ac_cv_have_decl_strtold='no'
ac_cv_header_arpa_inet_h='no'
ac_cv_header_dirent_dirent_h='yes'
ac_cv_header_dlfcn_h='no'
ac_cv_header_grp_h='no'
ac_cv_header_inttypes_h='no'
ac_cv_header_langinfo_h='no'
ac_cv_header_libintl_h='no'
ac_cv_header_limits_h='yes'
ac_cv_header_locale_h='yes'
ac_cv_header_memory_h='no'
ac_cv_header_minix_config_h='no'
ac_cv_header_netdb_h='no'
ac_cv_header_netinet_in_h='no'
ac_cv_header_stat_broken='no'
ac_cv_header_stdarg_h='yes'
ac_cv_header_stdc='no'
ac_cv_header_stddef_h='yes'
ac_cv_header_stdint_h='yes'
ac_cv_header_stdlib_h='yes'
ac_cv_header_string_h='yes'
ac_cv_header_strings_h='no'
ac_cv_header_sys_file_h='yes'
ac_cv_header_sys_param_h='no'
ac_cv_header_sys_pte_h='no'
ac_cv_header_sys_ptem_h='no'
ac_cv_header_sys_resource_h='no'
ac_cv_header_sys_select_h='no'
ac_cv_header_sys_socket_h='no'
ac_cv_header_sys_stat_h='yes'
ac_cv_header_sys_stream_h='no'
ac_cv_header_sys_time_h='yes'
ac_cv_header_sys_times_h='yes'
ac_cv_header_sys_types_h='yes'
ac_cv_header_sys_wait_h='yes'
ac_cv_header_termcap_h='no'
ac_cv_header_termio_h='no'
ac_cv_header_termios_h='yes'
ac_cv_header_time='yes'
ac_cv_header_unistd_h='yes'
ac_cv_header_varargs_h='no'
ac_cv_header_wchar_h='no'
ac_cv_header_wctype_h='no'
ac_cv_host='x86_64-unknown-none'
ac_cv_host_alias='x86_64-unknown-none'
ac_cv_lib_dir_opendir='no'
ac_cv_lib_dl_dlopen='no'
ac_cv_lib_intl_bindtextdomain='no'
ac_cv_lib_socket_getpeername='no'
ac_cv_member_struct_stat_st_blocks='yes'
ac_cv_member_struct_termio_c_line='no'
ac_cv_member_struct_termios_c_line='yes'
ac_cv_member_struct_tm_tm_zone='yes'
ac_cv_objext='o'
ac_cv_path_install='/usr/bin/install -c'
ac_cv_prog_AR='ar'
ac_cv_prog_ac_ct_RANLIB='ranlib'
ac_cv_prog_cc_g='yes'
ac_cv_prog_cc_stdc=''
ac_cv_prog_gcc_traditional='no'
ac_cv_prog_make_make_set='yes'
ac_cv_sizeof_char='1'
ac_cv_sizeof_char_p='8'
ac_cv_sizeof_double='8'
ac_cv_sizeof_int='4'
ac_cv_sizeof_long='8'
ac_cv_sizeof_long_long='8'
ac_cv_sizeof_short='2'
ac_cv_struct_tm='time.h'
ac_cv_sys_file_offset_bits='no'
ac_cv_sys_interpreter='yes'
ac_cv_sys_large_files='no'
ac_cv_sys_largefile_CC='no'
ac_cv_sys_posix_termios='yes'
ac_cv_sys_tiocgwinsz_in_sys_ioctl_h='yes'
ac_cv_sys_tiocgwinsz_in_termios_h='no'
ac_cv_type_bits16_t='yes'
ac_cv_type_bits32_t='yes'
ac_cv_type_bits64_t='yes'
ac_cv_type_char='yes'
ac_cv_type_char_p='yes'
ac_cv_type_double='yes'
ac_cv_type_getgroups='int'
ac_cv_type_int='yes'
ac_cv_type_long='yes'
ac_cv_type_long_long='yes'
ac_cv_type_mode_t='yes'
ac_cv_type_off_t='yes'
ac_cv_type_pid_t='yes'
ac_cv_type_ptrdiff_t='yes'
ac_cv_type_short='yes'
ac_cv_type_signal='void'
ac_cv_type_size_t='yes'
ac_cv_type_ssize_t='yes'
ac_cv_type_time_t='yes'
ac_cv_type_u_bits16_t='yes'
ac_cv_type_u_bits32_t='yes'
ac_cv_type_u_int='yes'
ac_cv_type_u_long='yes'
ac_cv_type_uid_t='yes'
ac_cv_working_alloca_h='no'
bash_cv_decl_strtoimax='no'
bash_cv_decl_strtol='no'
bash_cv_decl_strtoll='no'
bash_cv_decl_strtoul='no'
bash_cv_decl_strtoull='no'
bash_cv_decl_strtoumax='no'
bash_cv_decl_under_sys_siglist='no'
bash_cv_dev_fd='absent'
bash_cv_dev_stdin='absent'
bash_cv_dirent_has_d_fileno='no'
bash_cv_dirent_has_dino='yes'
bash_cv_dup2_broken='no'
bash_cv_fionread_in_ioctl='no'
bash_cv_func_inet_aton='no'
bash_cv_func_sigsetjmp='missing'
bash_cv_func_strcoll_broken='no'
bash_cv_getcwd_calls_popen='no'
bash_cv_getcwd_malloc='yes'
bash_cv_getenv_redef='yes'
bash_cv_getpw_declared='yes'
bash_cv_have_gethostbyname='no'
bash_cv_have_mbstate_t='no'
bash_cv_have_socklib='no'
bash_cv_have_strsignal='no'
bash_cv_job_control_missing='missing'
bash_cv_langinfo_codeset='no'
bash_cv_mail_dir='/var/mail'
bash_cv_must_reinstall_sighandlers='no'
bash_cv_opendir_not_robust='no'
bash_cv_pgrp_pipe='no'
bash_cv_printf_a_format='no'
bash_cv_signal_vintage='posix'
bash_cv_speed_t_in_sys_types='yes'
bash_cv_struct_timeval='yes'
bash_cv_struct_winsize_header='ioctl_h'
bash_cv_sys_errlist='yes'
bash_cv_sys_named_pipes='missing'
bash_cv_sys_siglist='no'
bash_cv_tiocstat_in_ioctl='no'
bash_cv_type_clock_t='yes'
bash_cv_type_intmax_t='yes'
bash_cv_type_long_long='long long'
bash_cv_type_quad_t='no'
bash_cv_type_rlimit='long'
bash_cv_type_sigset_t='yes'
bash_cv_type_uintmax_t='yes'
bash_cv_type_unsigned_long_long='unsigned long long'
bash_cv_ulimit_maxfds='no'
bash_cv_under_sys_siglist='no'
bash_cv_unusable_rtsigs='yes'
bash_cv_void_sighandler='yes'

export ac_cv_build
export ac_cv_build_alias
export ac_cv_c_bigendian
export ac_cv_c_char_unsigned
export ac_cv_c_compiler_gnu
export ac_cv_c_const
export ac_cv_c_inline
export ac_cv_c_long_double
export ac_cv_c_stringize
export ac_cv_decl_sys_siglist
export ac_cv_func___setostype
export ac_cv_func__doprnt
export ac_cv_func_alarm
export ac_cv_func_alloca_works
export ac_cv_func_asprintf
export ac_cv_func_bcopy
export ac_cv_func_bindtextdomain
export ac_cv_func_bzero
export ac_cv_func_confstr
export ac_cv_func_dlclose
export ac_cv_func_dlopen
export ac_cv_func_dlsym
export ac_cv_func_dup2
export ac_cv_func_fnmatch
export ac_cv_func_getaddrinfo
export ac_cv_func_getcwd
export ac_cv_func_getdtablesize
export ac_cv_func_getgroups
export ac_cv_func_gethostbyname
export ac_cv_func_gethostname
export ac_cv_func_getpagesize
export ac_cv_func_getpeername
export ac_cv_func_getpgrp_void
export ac_cv_func_getrlimit
export ac_cv_func_getrusage
export ac_cv_func_getservbyname
export ac_cv_func_getservent
export ac_cv_func_gettext
export ac_cv_func_gettimeofday
export ac_cv_func_inet_aton
export ac_cv_func_isascii
export ac_cv_func_isblank
export ac_cv_func_isgraph
export ac_cv_func_isinf
export ac_cv_func_isprint
export ac_cv_func_isspace
export ac_cv_func_isxdigit
export ac_cv_func_killpg
export ac_cv_func_lstat
export ac_cv_func_mbsrtowcs
export ac_cv_func_memmove
export ac_cv_func_memset
export ac_cv_func_mkfifo
export ac_cv_func_pathconf
export ac_cv_func_putenv
export ac_cv_func_readlink
export ac_cv_func_rename
export ac_cv_func_sbrk
export ac_cv_func_select
export ac_cv_func_setdtablesize
export ac_cv_func_setenv
export ac_cv_func_setlinebuf
export ac_cv_func_setlocale
export ac_cv_func_setvbuf
export ac_cv_func_setvbuf_reversed
export ac_cv_func_siginterrupt
export ac_cv_func_snprintf
export ac_cv_func_strcasecmp
export ac_cv_func_strchr
export ac_cv_func_strcoll_works
export ac_cv_func_strerror
export ac_cv_func_strftime
export ac_cv_func_strnlen
export ac_cv_func_strpbrk
export ac_cv_func_strstr
export ac_cv_func_strdup
export ac_cv_func_strtod
export ac_cv_func_strtoimax
export ac_cv_func_strtol
export ac_cv_func_strtoll
export ac_cv_func_strtoul
export ac_cv_func_strtoull
export ac_cv_func_strtoumax
export ac_cv_func_sysconf
export ac_cv_func_tcgetattr
export ac_cv_func_tcgetpgrp
export ac_cv_func_textdomain
export ac_cv_func_times
export ac_cv_func_ttyname
export ac_cv_func_tzset
export ac_cv_func_ulimit
export ac_cv_func_uname
export ac_cv_func_unsetenv
export ac_cv_func_vasprintf
export ac_cv_func_vprintf
export ac_cv_func_vsnprintf
export ac_cv_func_wait3
export ac_cv_func_waitpid
export ac_cv_func_wcwidth
export ac_cv_func_working_mktime
export ac_cv_have_decl_confstr
export ac_cv_have_decl_printf
export ac_cv_have_decl_sbrk
export ac_cv_have_decl_strcpy
export ac_cv_have_decl_strsignal
export ac_cv_have_decl_strtold
export ac_cv_header_arpa_inet_h
export ac_cv_header_dirent_dirent_h
export ac_cv_header_dlfcn_h
export ac_cv_header_grp_h
export ac_cv_header_inttypes_h
export ac_cv_header_langinfo_h
export ac_cv_header_libintl_h
export ac_cv_header_limits_h
export ac_cv_header_locale_h
export ac_cv_header_memory_h
export ac_cv_header_minix_config_h
export ac_cv_header_netdb_h
export ac_cv_header_netinet_in_h
export ac_cv_header_stat_broken
export ac_cv_header_stdarg_h
export ac_cv_header_stdc
export ac_cv_header_stddef_h
export ac_cv_header_stdint_h
export ac_cv_header_stdlib_h
export ac_cv_header_string_h
export ac_cv_header_strings_h
export ac_cv_header_sys_file_h
export ac_cv_header_sys_param_h
export ac_cv_header_sys_pte_h
export ac_cv_header_sys_ptem_h
export ac_cv_header_sys_resource_h
export ac_cv_header_sys_select_h
export ac_cv_header_sys_socket_h
export ac_cv_header_sys_stat_h
export ac_cv_header_sys_stream_h
export ac_cv_header_sys_time_h
export ac_cv_header_sys_times_h
export ac_cv_header_sys_types_h
export ac_cv_header_sys_wait_h
export ac_cv_header_termcap_h
export ac_cv_header_termio_h
export ac_cv_header_termios_h
export ac_cv_header_time
export ac_cv_header_unistd_h
export ac_cv_header_varargs_h
export ac_cv_header_wchar_h
export ac_cv_header_wctype_h
export ac_cv_host
export ac_cv_host_alias
export ac_cv_lib_dir_opendir
export ac_cv_lib_dl_dlopen
export ac_cv_lib_intl_bindtextdomain
export ac_cv_lib_socket_getpeername
export ac_cv_member_struct_stat_st_blocks
export ac_cv_member_struct_termio_c_line
export ac_cv_member_struct_termios_c_line
export ac_cv_member_struct_tm_tm_zone
export ac_cv_objext
export ac_cv_path_install
export ac_cv_prog_AR
export ac_cv_prog_ac_ct_RANLIB
export ac_cv_prog_cc_g
export ac_cv_prog_cc_stdc
export ac_cv_prog_gcc_traditional
export ac_cv_prog_make_make_set
export ac_cv_sizeof_char
export ac_cv_sizeof_char_p
export ac_cv_sizeof_double
export ac_cv_sizeof_int
export ac_cv_sizeof_long
export ac_cv_sizeof_long_long
export ac_cv_sizeof_short
export ac_cv_struct_tm
export ac_cv_sys_file_offset_bits
export ac_cv_sys_interpreter
export ac_cv_sys_large_files
export ac_cv_sys_largefile_CC
export ac_cv_sys_posix_termios
export ac_cv_sys_tiocgwinsz_in_sys_ioctl_h
export ac_cv_sys_tiocgwinsz_in_termios_h
export ac_cv_type_bits16_t
export ac_cv_type_bits32_t
export ac_cv_type_bits64_t
export ac_cv_type_char
export ac_cv_type_char_p
export ac_cv_type_double
export ac_cv_type_getgroups
export ac_cv_type_int
export ac_cv_type_long
export ac_cv_type_long_long
export ac_cv_type_mode_t
export ac_cv_type_off_t
export ac_cv_type_pid_t
export ac_cv_type_ptrdiff_t
export ac_cv_type_short
export ac_cv_type_signal
export ac_cv_type_size_t
export ac_cv_type_ssize_t
export ac_cv_type_time_t
export ac_cv_type_u_bits16_t
export ac_cv_type_u_bits32_t
export ac_cv_type_u_int
export ac_cv_type_u_long
export ac_cv_type_uid_t
export ac_cv_working_alloca_h
export bash_cv_decl_strtoimax
export bash_cv_decl_strtol
export bash_cv_decl_strtoll
export bash_cv_decl_strtoul
export bash_cv_decl_strtoull
export bash_cv_decl_strtoumax
export bash_cv_decl_under_sys_siglist
export bash_cv_dev_fd
export bash_cv_dev_stdin
export bash_cv_dirent_has_d_fileno
export bash_cv_dirent_has_dino
export bash_cv_dup2_broken
export bash_cv_fionread_in_ioctl
export bash_cv_func_inet_aton
export bash_cv_func_sigsetjmp
export bash_cv_func_strcoll_broken
export bash_cv_getcwd_calls_popen
export bash_cv_getcwd_malloc
export bash_cv_getenv_redef
export bash_cv_getpw_declared
export bash_cv_have_gethostbyname
export bash_cv_have_mbstate_t
export bash_cv_have_socklib
export bash_cv_have_strsignal
export bash_cv_job_control_missing
export bash_cv_langinfo_codeset
export bash_cv_mail_dir
export bash_cv_must_reinstall_sighandlers
export bash_cv_opendir_not_robust
export bash_cv_pgrp_pipe
export bash_cv_printf_a_format
export bash_cv_signal_vintage
export bash_cv_speed_t_in_sys_types
export bash_cv_struct_timeval
export bash_cv_struct_winsize_header
export bash_cv_sys_errlist
export bash_cv_sys_named_pipes
export bash_cv_sys_siglist
export bash_cv_tiocstat_in_ioctl
export bash_cv_type_clock_t
export bash_cv_type_intmax_t
export bash_cv_type_long_long
export bash_cv_type_quad_t
export bash_cv_type_rlimit
export bash_cv_type_sigset_t
export bash_cv_type_uintmax_t
export bash_cv_type_unsigned_long_long
export bash_cv_ulimit_maxfds
export bash_cv_under_sys_siglist
export bash_cv_unusable_rtsigs
export bash_cv_void_sighandler
