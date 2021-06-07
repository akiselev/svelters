const { loadBinding } = require('@node-rs/helper')

/**
 * __dirname means load native addon from current dir
 * 'svelte-qt' means native addon name is `svelte-qt`
 * the first arguments was decided by `napi.name` field in `package.json`
 * the second arguments was decided by `name` field in `package.json`
 * loadBinding helper will load `svelte-qt.[PLATFORM].node` from `__dirname` first
 * If failed to load addon, it will fallback to load from `svelte-qt-[PLATFORM]`
 */
// module.exports = loadBinding(__dirname, 'svelte-qt', 'svelte-qt')
console.log('Loading rust library....')
const { QMainWindow, QApplication, QPushButton } = loadBinding(__dirname, 'svelte-qt', 'svelte-qt')
console.log('Creating application....')
const app = new QApplication()
app.exec()
console.log('Creating main window....')
const mainWindow = new QMainWindow()
const button = new QPushButton()
mainWindow.setCentralWidget(button)
console.log('Showing window....')
mainWindow.show()

setTimeout(() => console.log('Finished'), 5000)
