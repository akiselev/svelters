export type SvelteQt = {
    QApplication: typeof QApplication;
    QMainWindow: typeof QMainWindow;
    QPushButton: typeof QPushButton;
    QWidget: typeof QWidget;
};

export declare class QObject {}

export declare class QWidget extends QObject {}

export declare class QApplication extends QObject {
    constructor();
    exec(): void;
}

export declare class QMainWindow extends QWidget {
    constructor();
    show(): void;
    setCentralWidget(widget: QWidget): void;
}

export declare class QPushButton extends QWidget {
    constructor();
    show(): void;
    onClicked(fn: (val: boolean | null) => void): void;
}
