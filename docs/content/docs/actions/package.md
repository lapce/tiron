+++
title = "package"
template = "docs/section.html"
+++

# package

Install packages

### Parameters

| Parameter      | Description |
| -------------- | ----------- |
| **name** <br> String or List of String <br>Required: true | the name of the packages to be installed |
| **state** <br> Enum of "present", "absent", "latest" <br>Required: true | Whether to install or remove or update packages<br>`present` to install<br>`absent` to remove<br>`latest` to update |
