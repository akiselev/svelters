import * as mod from "QtCore";
window.main = new mod.QMainWindow();
console.log("111");
const button = new mod.QPushButton();
console.log("222");
button.text = "TEST TEXT";
window.main.setCentralWidget(button);
console.log("333");

window.main.show();
