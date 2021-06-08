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
    insertBefore(widget: QWidget, anchor: QWidget | null): void;
}

export declare class QPushButton extends QWidget {
    constructor();
    show(): void;
    addEventListener(name: string, fn: (val: any) => void): void;
    removeEventListener(name: string, fn: any): void;
    setAttribute(attr: string, val: any): void;
    getAttribute(attr: string): any;
}
