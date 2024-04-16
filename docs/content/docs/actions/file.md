+++
title = "file"
template = "docs/section.html"
+++

# file

Manage files/folders and their properties

### Parameters

| Parameter      | Description |
| -------------- | ----------- |
| **path** <br> String <br>Required: true | Path of the file or folder that's managed |
| **state** <br> Enum of "file", "absent", "directory" <br>Required: false | Default to `file`<br><br>If `file`, a file will be managed.<br>If `directory`, a directory will be recursively created and all of its parent components if they are missing.<br>If `absent`, directories will be recursively deleted and all its contents, and files or symlinks will be unlinked. |
