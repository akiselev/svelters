// Type-checks the generated Qt typings against the ergonomics of the plan (section 7, N2 Qt twins).
import { QTimer, QStringListModel, QUrl, QFileInfo, QMimeDatabase, QTemporaryDir, QModelIndex } from 'svelters:qt/core';
import { QTextDocument, QStandardItemModel, QStandardItem, QIcon, QFont } from 'svelters:qt/gui';

const doc = new QTextDocument({ plainText: 'Hello' });
const plain: string = doc.toPlainText();
const off: () => void = doc.on('contentsChanged', () => {});
off();

const timer = new QTimer({ interval: 500, singleShot: true });
timer.interval = 250;
const active: boolean = timer.active;
timer.start();
timer.start(100);
timer.on('timeout', () => timer.stop());

const names = new QStringListModel();
names.setStringList(['one', 'two']);
const rows: number = names.rowCount();
const idx: QModelIndex = names.index(0, 0);
const row: number = idx.row();

const model = new QStandardItemModel();
const item = new QStandardItem('a');
model.appendRow(item);
const back: QStandardItem | null = model.item(0);

const url = new QUrl('https://example.org/a');
const host: string = url.host();
const same: boolean = url.equals(QUrl.fromLocalFile('/tmp'));
const local: QUrl = QUrl.fromLocalFile('/tmp');

const info = new QFileInfo('/usr/share');
const isDir: boolean = info.isDir();
const mime = new QMimeDatabase();
const tmp = new QTemporaryDir();
const icon = QIcon.fromTheme('document-open');
const font = new QFont('Sans', 12);

export const used = [plain, active, rows, row, back, host, same, local, isDir, mime, tmp, icon, font];
