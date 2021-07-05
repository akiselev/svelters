Feature: Core

    Scenario: testing runner
        Given module at "test.js"
        When calling function "testConsole"

    Scenario: testing return value
        Given module at "test.js"
        When calling function "testReturnValue" with args ("Runner")
        Then return value is "Hello Runner"

    Scenario: testing args
        Given module at "test.js"
        When calling function "testArgs" with args ("One", "Two", "Three", "Four", "Five")
        Then return value is "One Two Three Four Five"

    Scenario: check that Object constructor is a function
        Given module at "core.js"
        When calling function "getTypeOf" with args ("Object")
        Then return value is "function"

    Scenario: check that fake prop is undefined
        Given module at "core.js"
        When calling function "getTypeOf" with args ("Object", "notReal")
        Then return value is "undefined"

    Scenario: check get class name
        Given module at "core.js"
        When calling function "getClassName"
        Then return value is "Object"

    Scenario: check basic method call
        Given module at "core.js"
        When calling function "callMethod" with args ("Object", "className")
        Then return value is "Object"

    Scenario: check has method
        Given module at "core.js"
        When calling function "hasMethod" with args ("Object", "className")
        Then return value is true


    Scenario: check that the className getter is overriden by each child class
        Given module at "core.js"
        When calling function "getClassName" with args (<class>)
        Then return value is <result>

        Examples:
            | class         | result        |
            | "Object"      | "Object"      |
            | "Application" | "Application" |
            | "Widget"      | "Widget"      |
            | "MainWindow"  | "MainWindow"  |
            | "BoxLayout"   | "BoxLayout"   |
            | "PushButton"  | "PushButton"  |

    Scenario: construct class
        Given module at "core.js"
        When calling function "constructClass" with args ("Object")
        Then return value is object
# Then that string is now equal to "something interesting"

# Scenario: async things
#     Given some awkward async process to get a string
#     And some tables being filled with data
#     When we check a database we have prefilled with a string
#     Then we find we somehow had the same string

# Scenario Outline: eating
#     Given there are <start> cucumbers
#     When I eat <eat> cucumbers
#     Then I should have <left> cucumbers

#     Examples:
#         | start | eat | left |
#         | 12    | 5   | 7    |
#         | 20    | 5   | 15   |
