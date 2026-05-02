## Testing

To successfully include tags when creating a tar archive on macOS, you should use the -p (preserve permissions) and -X (or standard behavior of bsdtar) flags to include extended attributes.

```shell
tar -cpcvf archive_name.tar.gz /path/to/directory
```