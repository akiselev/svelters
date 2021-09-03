Feature: Core
# Scenario: Create element
#     Given module at "core.js"
#     When calling function "callElementMethod" with args ("mainWindow")
#     Then return value is "MainWindow"

# Scenario: Create element in bulk
#     Given module at "core.js"
#     When calling function "callElementMethod" with args (<class>, "className2")
#     Then return value is <result>

#     Examples:
#         | class        | result           |
#         | "mainWindow" | "MainWindow"     |
#         | "boxLayout"  | "Widget"         |
#         | "pushButton" | "AbstractButton" |

# Scenario: Get element readonly
#     Given module at "core.js"
#     When calling function "getElementProp" with args ("mainWindow", "ro")
#     Then return value is "Readonly Hello Widget"