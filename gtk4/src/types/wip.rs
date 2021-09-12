use crate::inner::*;
use gtk4::*;

define!(|cls: ActionBar| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("actionBar", || Ok(Gtk(ActionBar::new())))?;
    props!(cls (this: Gtk<ActionBar> ) {

    });
    methods!(cls ( this: Gtk<ActionBar> ) {

    });
});

define!(|cls: Actionable| {
    let cls = cls.superclass::<Gtk<Widget>>()?;
    // .as_element("actionable", || Ok(Gtk(Actionable::new())))?;
    props!(cls (this: Gtk<Actionable> ) {

    });
    methods!(cls ( this: Gtk<Actionable> ) {

    });
});

define!(|cls: AppChooserButton| {
    let cls = cls.superclass::<Gtk<Widget>>()?;
    // .as_element("appChooserButton", || Ok(Gtk(AppChooserButton::new())))?;
    props!(cls (this: Gtk<AppChooserButton> ) {

    });
    methods!(cls ( this: Gtk<AppChooserButton> ) {

    });
});

define!(|cls: AppChooserWidget| {
    let cls = cls.superclass::<Gtk<Widget>>()?;
    // .as_element("appChooserWidget", || Ok(Gtk(AppChooserWidget::new())))?;
    props!(cls (this: Gtk<AppChooserWidget> ) {

    });
    methods!(cls ( this: Gtk<AppChooserWidget> ) {

    });
});

define!(|cls: AppChooser| {
    let cls = cls.superclass::<Gtk<Widget>>()?;
    // .as_element("appChooser", || Ok(Gtk(AppChooser::new())))?;
    props!(cls (this: Gtk<AppChooser> ) {

    });
    methods!(cls ( this: Gtk<AppChooser> ) {

    });
});

define!(|cls: AspectFrame| {
    let cls = cls.superclass::<Gtk<Widget>>()?;
    // .as_element("aspectFrame", || Ok(Gtk(AspectFrame::new())))?;
    props!(cls (this: Gtk<AspectFrame> ) {

    });
    methods!(cls ( this: Gtk<AspectFrame> ) {

    });
});

define!(|cls: Box| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("vBox", || Ok(Gtk(Box::new(Orientation::Vertical, 0))))?
        .as_element("hBox", || Ok(Gtk(Box::new(Orientation::Horizontal, 0))))?;
    props!(cls (this: Gtk<Box> ) {

    });
    methods!(cls ( this: Gtk<Box> ) {

    });
});

define!(|cls: Calendar| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("calendar", || Ok(Gtk(Calendar::new())))?;
    props!(cls (this: Gtk<Calendar> ) {

    });
    methods!(cls ( this: Gtk<Calendar> ) {

    });
});

define!(|cls: CellEditable| {
    let cls = cls.superclass::<Gtk<Widget>>()?;
    // .as_element("cellEditable", || Ok(Gtk(CellEditable::new())))?;
    props!(cls (this: Gtk<CellEditable> ) {

    });
    methods!(cls ( this: Gtk<CellEditable> ) {

    });
});

define!(|cls: CellView| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("cellView", || Ok(Gtk(CellView::new())))?;
    props!(cls (this: Gtk<CellView> ) {

    });
    methods!(cls ( this: Gtk<CellView> ) {

    });
});

define!(|cls: CenterBox| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("centerBox", || Ok(Gtk(CenterBox::new())))?;
    props!(cls (this: Gtk<CenterBox> ) {

    });
    methods!(cls ( this: Gtk<CenterBox> ) {

    });
});

define!(|cls: CheckButton| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("checkButton", || Ok(Gtk(CheckButton::new())))?;
    props!(cls (this: Gtk<CheckButton> ) {

    });
    methods!(cls ( this: Gtk<CheckButton> ) {

    });
});

define!(|cls: ColorButton| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("colorButton", || Ok(Gtk(ColorButton::new())))?;
    props!(cls (this: Gtk<ColorButton> ) {

    });
    methods!(cls ( this: Gtk<ColorButton> ) {

    });
});

define!(|cls: ColorChooserWidget| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("colorChooserWidget", || Ok(Gtk(ColorChooserWidget::new())))?;
    props!(cls (this: Gtk<ColorChooserWidget> ) {

    });
    methods!(cls ( this: Gtk<ColorChooserWidget> ) {

    });
});

define!(|cls: ColumnView| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("columnView", || {
            Ok(Gtk(ColumnView::new::<SelectionModel>(None)))
        })?;
    props!(cls (this: Gtk<ColumnView> ) {

    });
    methods!(cls ( this: Gtk<ColumnView> ) {

    });
});

define!(|cls: ComboBox| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("comboBox", || Ok(Gtk(ComboBox::new())))?;
    props!(cls (this: Gtk<ComboBox> ) {

    });
    methods!(cls ( this: Gtk<ComboBox> ) {

    });
});

define!(|cls: DragIcon| {
    let cls = cls.superclass::<Gtk<Widget>>()?;
    // .as_element("dragIcon", || Ok(Gtk(DragIcon::new())))?;
    props!(cls (this: Gtk<DragIcon> ) {

    });
    methods!(cls ( this: Gtk<DragIcon> ) {

    });
});

define!(|cls: DrawingArea| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("drawingArea", || Ok(Gtk(DrawingArea::new())))?;
    props!(cls (this: Gtk<DrawingArea> ) {

    });
    methods!(cls ( this: Gtk<DrawingArea> ) {

    });
});

define!(|cls: DropDown| {
    let cls = cls.superclass::<Gtk<Widget>>()?;
    // .as_element("dropDown", || Ok(Gtk(DropDown::new(None, None))))?;
    props!(cls (this: Gtk<DropDown> ) {

    });
    methods!(cls ( this: Gtk<DropDown> ) {

    });
});

define!(|cls: EditableLabel| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("editableLabel", || Ok(Gtk(EditableLabel::new(""))))?;
    props!(cls (this: Gtk<EditableLabel> ) {

    });
    methods!(cls ( this: Gtk<EditableLabel> ) {

    });
});

define!(|cls: Editable| {
    let cls = cls.superclass::<Gtk<Widget>>()?;
    // .as_element("editable", || Ok(Gtk(Editable::new())))?;
    props!(cls (this: Gtk<Editable> ) {

    });
    methods!(cls ( this: Gtk<Editable> ) {

    });
});

define!(|cls: Entry| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("entry", || Ok(Gtk(Entry::new())))?;
    props!(cls (this: Gtk<Entry> ) {

    });
    methods!(cls ( this: Gtk<Entry> ) {

    });
});

define!(|cls: Expander| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("expander", || Ok(Gtk(Expander::new(None))))?;
    props!(cls (this: Gtk<Expander> ) {

    });
    methods!(cls ( this: Gtk<Expander> ) {

    });
});

define!(|cls: FileChooserWidget| {
    let cls = cls.superclass::<Gtk<Widget>>()?;
    // .as_element("fileChooserWidget", || Ok(Gtk(FileChooserWidget::new())))?;
    props!(cls (this: Gtk<FileChooserWidget> ) {

    });
    methods!(cls ( this: Gtk<FileChooserWidget> ) {

    });
});

define!(|cls: Fixed| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("fixed", || Ok(Gtk(Fixed::new())))?;
    props!(cls (this: Gtk<Fixed> ) {

    });
    methods!(cls ( this: Gtk<Fixed> ) {

    });
});

define!(|cls: FlowBoxChild| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("flowBoxChild", || Ok(Gtk(FlowBoxChild::new())))?;
    props!(cls (this: Gtk<FlowBoxChild> ) {

    });
    methods!(cls ( this: Gtk<FlowBoxChild> ) {

    });
});

define!(|cls: FlowBox| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("flowBox", || Ok(Gtk(FlowBox::new())))?;
    props!(cls (this: Gtk<FlowBox> ) {

    });
    methods!(cls ( this: Gtk<FlowBox> ) {

    });
});

define!(|cls: FontButton| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("fontButton", || Ok(Gtk(FontButton::new())))?;
    props!(cls (this: Gtk<FontButton> ) {

    });
    methods!(cls ( this: Gtk<FontButton> ) {

    });
});

define!(|cls: FontChooserWidget| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("fontChooserWidget", || Ok(Gtk(FontChooserWidget::new())))?;
    props!(cls (this: Gtk<FontChooserWidget> ) {

    });
    methods!(cls ( this: Gtk<FontChooserWidget> ) {

    });
});

define!(|cls: Frame| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("frame", || Ok(Gtk(Frame::new(None))))?;
    props!(cls (this: Gtk<Frame> ) {

    });
    methods!(cls ( this: Gtk<Frame> ) {

    });
});

define!(|cls: GLArea| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("gLArea", || Ok(Gtk(GLArea::new())))?;
    props!(cls (this: Gtk<GLArea> ) {

    });
    methods!(cls ( this: Gtk<GLArea> ) {

    });
});

define!(|cls: Grid| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("grid", || Ok(Gtk(Grid::new())))?;
    props!(cls (this: Gtk<Grid> ) {

    });
    methods!(cls ( this: Gtk<Grid> ) {

    });
});

define!(|cls: HeaderBar| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("headerBar", || Ok(Gtk(HeaderBar::new())))?;
    props!(cls (this: Gtk<HeaderBar> ) {

    });
    methods!(cls ( this: Gtk<HeaderBar> ) {

    });
});

define!(|cls: IconView| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("iconView", || Ok(Gtk(IconView::new())))?;
    props!(cls (this: Gtk<IconView> ) {

    });
    methods!(cls ( this: Gtk<IconView> ) {

    });
});

define!(|cls: Image| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("image", || Ok(Gtk(Image::new())))?;
    props!(cls (this: Gtk<Image> ) {

    });
    methods!(cls ( this: Gtk<Image> ) {

    });
});

define!(|cls: InfoBar| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("infoBar", || Ok(Gtk(InfoBar::new())))?;
    props!(cls (this: Gtk<InfoBar> ) {

    });
    methods!(cls ( this: Gtk<InfoBar> ) {

    });
});

define!(|cls: Label| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("label", || Ok(Gtk(Label::new(None))))?;
    props!(cls (this: Gtk<Label> ) {

    });
    methods!(cls ( this: Gtk<Label> ) {

    });
});

define!(|cls: LevelBar| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("levelBar", || Ok(Gtk(LevelBar::new())))?;
    props!(cls (this: Gtk<LevelBar> ) {

    });
    methods!(cls ( this: Gtk<LevelBar> ) {

    });
});

define!(|cls: ListBase| {
    let cls = cls.superclass::<Gtk<Widget>>()?;
    // .as_element("listBase", || Ok(Gtk(ListBase::new())))?;
    props!(cls (this: Gtk<ListBase> ) {

    });
    methods!(cls ( this: Gtk<ListBase> ) {

    });
});

define!(|cls: ListBoxRow| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("listBoxRow", || Ok(Gtk(ListBoxRow::new())))?;
    props!(cls (this: Gtk<ListBoxRow> ) {

    });
    methods!(cls ( this: Gtk<ListBoxRow> ) {

    });
});

define!(|cls: ListBox| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("listBox", || Ok(Gtk(ListBox::new())))?;
    props!(cls (this: Gtk<ListBox> ) {

    });
    methods!(cls ( this: Gtk<ListBox> ) {

    });
});

define!(|cls: MediaControls| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("mediaControls", || {
            Ok(Gtk(MediaControls::new::<MediaStream>(None)))
        })?;
    props!(cls (this: Gtk<MediaControls> ) {

    });
    methods!(cls ( this: Gtk<MediaControls> ) {

    });
});

define!(|cls: MenuButton| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("menuButton", || Ok(Gtk(MenuButton::new())))?;
    props!(cls (this: Gtk<MenuButton> ) {

    });
    methods!(cls ( this: Gtk<MenuButton> ) {

    });
});

define!(|cls: Native| {
    let cls = cls.superclass::<Gtk<Widget>>()?;
    // .as_element("native", || Ok(Gtk(Native::new())))?;
    props!(cls (this: Gtk<Native> ) {

    });
    methods!(cls ( this: Gtk<Native> ) {

    });
});

define!(|cls: Notebook| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("notebook", || Ok(Gtk(Notebook::new())))?;
    props!(cls (this: Gtk<Notebook> ) {

    });
    methods!(cls ( this: Gtk<Notebook> ) {

    });
});

define!(|cls: Overlay| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("overlay", || Ok(Gtk(Overlay::new())))?;
    props!(cls (this: Gtk<Overlay> ) {

    });
    methods!(cls ( this: Gtk<Overlay> ) {

    });
});

define!(|cls: Paned| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("vPaned", || Ok(Gtk(Paned::new(Orientation::Vertical))))?
        .as_element("hPaned", || Ok(Gtk(Paned::new(Orientation::Horizontal))))?;
    props!(cls (this: Gtk<Paned> ) {

    });
    methods!(cls ( this: Gtk<Paned> ) {

    });
});

define!(|cls: PasswordEntry| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("passwordEntry", || Ok(Gtk(PasswordEntry::new())))?;
    props!(cls (this: Gtk<PasswordEntry> ) {

    });
    methods!(cls ( this: Gtk<PasswordEntry> ) {

    });
});

define!(|cls: Picture| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("picture", || Ok(Gtk(Picture::new())))?;
    props!(cls (this: Gtk<Picture> ) {

    });
    methods!(cls ( this: Gtk<Picture> ) {

    });
});

define!(|cls: PopoverMenuBar| {
    let cls = cls.superclass::<Gtk<Widget>>()?;
    // .as_element("popoverMenuBar", || Ok(Gtk(PopoverMenuBar::new())))?;
    props!(cls (this: Gtk<PopoverMenuBar> ) {

    });
    methods!(cls ( this: Gtk<PopoverMenuBar> ) {

    });
});

define!(|cls: Popover| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("popover", || Ok(Gtk(Popover::new())))?;
    props!(cls (this: Gtk<Popover> ) {

    });
    methods!(cls ( this: Gtk<Popover> ) {

    });
});

define!(|cls: ProgressBar| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("progressBar", || Ok(Gtk(ProgressBar::new())))?;
    props!(cls (this: Gtk<ProgressBar> ) {

    });
    methods!(cls ( this: Gtk<ProgressBar> ) {

    });
});

define!(|cls: Range| {
    let cls = cls.superclass::<Gtk<Widget>>()?;
    // .as_element("range", || Ok(Gtk(Range::new())))?;
    props!(cls (this: Gtk<Range> ) {

    });
    methods!(cls ( this: Gtk<Range> ) {

    });
});

define!(|cls: Revealer| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("revealer", || Ok(Gtk(Revealer::new())))?;
    props!(cls (this: Gtk<Revealer> ) {

    });
    methods!(cls ( this: Gtk<Revealer> ) {

    });
});

define!(|cls: Root| {
    let cls = cls.superclass::<Gtk<Widget>>()?;
    // .as_element("root", || Ok(Gtk(Root::new())))?;
    props!(cls (this: Gtk<Root> ) {

    });
    methods!(cls ( this: Gtk<Root> ) {

    });
});

define!(|cls: ScaleButton| {
    let cls = cls.superclass::<Gtk<Widget>>()?;
    // .as_element("scaleButton", || Ok(Gtk(ScaleButton::new())))?;
    props!(cls (this: Gtk<ScaleButton> ) {

    });
    methods!(cls ( this: Gtk<ScaleButton> ) {

    });
});

define!(|cls: Scrollbar| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("vScrollbar", || {
            Ok(Gtk(Scrollbar::new::<Adjustment>(
                Orientation::Vertical,
                None,
            )))
        })?
        .as_element("hScrollbar", || {
            Ok(Gtk(Scrollbar::new::<Adjustment>(
                Orientation::Horizontal,
                None,
            )))
        })?;
    props!(cls (this: Gtk<Scrollbar> ) {

    });
    methods!(cls ( this: Gtk<Scrollbar> ) {

    });
});

define!(|cls: ScrolledWindow| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("scrolledWindow", || Ok(Gtk(ScrolledWindow::new())))?;
    props!(cls (this: Gtk<ScrolledWindow> ) {

    });
    methods!(cls ( this: Gtk<ScrolledWindow> ) {

    });
});

define!(|cls: SearchBar| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("searchBar", || Ok(Gtk(SearchBar::new())))?;
    props!(cls (this: Gtk<SearchBar> ) {

    });
    methods!(cls ( this: Gtk<SearchBar> ) {

    });
});

define!(|cls: SearchEntry| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("searchEntry", || Ok(Gtk(SearchEntry::new())))?;
    props!(cls (this: Gtk<SearchEntry> ) {

    });
    methods!(cls ( this: Gtk<SearchEntry> ) {

    });
});

define!(|cls: Separator| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("vSeparator", || {
            Ok(Gtk(Separator::new(Orientation::Vertical)))
        })?
        .as_element("hSeparator", || {
            Ok(Gtk(Separator::new(Orientation::Horizontal)))
        })?;
    props!(cls (this: Gtk<Separator> ) {

    });
    methods!(cls ( this: Gtk<Separator> ) {

    });
});

define!(|cls: ShortcutLabel| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("shortcutLabel", || Ok(Gtk(ShortcutLabel::new(""))))?;
    props!(cls (this: Gtk<ShortcutLabel> ) {

    });
    methods!(cls ( this: Gtk<ShortcutLabel> ) {

    });
});

define!(|cls: ShortcutsShortcut| {
    let cls = cls.superclass::<Gtk<Widget>>()?;
    // .as_element("shortcutsShortcut", || Ok(Gtk(ShortcutsShortcut::new())))?;
    props!(cls (this: Gtk<ShortcutsShortcut> ) {

    });
    methods!(cls ( this: Gtk<ShortcutsShortcut> ) {

    });
});

define!(|cls: SpinButton| {
    let cls = cls.superclass::<Gtk<Widget>>()?;
    // .as_element("spinButton", || Ok(Gtk(SpinButton::new())))?;
    props!(cls (this: Gtk<SpinButton> ) {

    });
    methods!(cls ( this: Gtk<SpinButton> ) {

    });
});

define!(|cls: Spinner| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("spinner", || Ok(Gtk(Spinner::new())))?;
    props!(cls (this: Gtk<Spinner> ) {

    });
    methods!(cls ( this: Gtk<Spinner> ) {

    });
});

define!(|cls: StackSidebar| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("stackSidebar", || Ok(Gtk(StackSidebar::new())))?;
    props!(cls (this: Gtk<StackSidebar> ) {

    });
    methods!(cls ( this: Gtk<StackSidebar> ) {

    });
});

define!(|cls: StackSwitcher| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("stackSwitcher", || Ok(Gtk(StackSwitcher::new())))?;
    props!(cls (this: Gtk<StackSwitcher> ) {

    });
    methods!(cls ( this: Gtk<StackSwitcher> ) {

    });
});

define!(|cls: Stack| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("stack", || Ok(Gtk(Stack::new())))?;
    props!(cls (this: Gtk<Stack> ) {

    });
    methods!(cls ( this: Gtk<Stack> ) {

    });
});

define!(|cls: Statusbar| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("statusbar", || Ok(Gtk(Statusbar::new())))?;
    props!(cls (this: Gtk<Statusbar> ) {

    });
    methods!(cls ( this: Gtk<Statusbar> ) {

    });
});

define!(|cls: Switch| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("switch", || Ok(Gtk(Switch::new())))?;
    props!(cls (this: Gtk<Switch> ) {

    });
    methods!(cls ( this: Gtk<Switch> ) {

    });
});

define!(|cls: TextView| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("textView", || Ok(Gtk(TextView::new())))?;
    props!(cls (this: Gtk<TextView> ) {

    });
    methods!(cls ( this: Gtk<TextView> ) {

    });
});

define!(|cls: Text| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("text", || Ok(Gtk(Text::new())))?;
    props!(cls (this: Gtk<Text> ) {

    });
    methods!(cls ( this: Gtk<Text> ) {

    });
});

define!(|cls: TreeExpander| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("treeExpander", || Ok(Gtk(TreeExpander::new())))?;
    props!(cls (this: Gtk<TreeExpander> ) {

    });
    methods!(cls ( this: Gtk<TreeExpander> ) {

    });
});

define!(|cls: TreeView| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("treeView", || Ok(Gtk(TreeView::new())))?;
    props!(cls (this: Gtk<TreeView> ) {

    });
    methods!(cls ( this: Gtk<TreeView> ) {

    });
});

define!(|cls: Video| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("video", || Ok(Gtk(Video::new())))?;
    props!(cls (this: Gtk<Video> ) {

    });
    methods!(cls ( this: Gtk<Video> ) {

    });
});

define!(|cls: Viewport| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("viewport", || {
            Ok(Gtk(Viewport::new::<Adjustment, Adjustment>(None, None)))
        })?;
    props!(cls (this: Gtk<Viewport> ) {

    });
    methods!(cls ( this: Gtk<Viewport> ) {

    });
});

define!(|cls: WindowControls| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("windowControls", || {
            Ok(Gtk(WindowControls::new(PackType::Start)))
        })?;
    props!(cls (this: Gtk<WindowControls> ) {

    });
    methods!(cls ( this: Gtk<WindowControls> ) {

    });
});

define!(|cls: WindowHandle| {
    let cls = cls
        .superclass::<Gtk<Widget>>()?
        .as_element("windowHandle", || Ok(Gtk(WindowHandle::new())))?;
    props!(cls (this: Gtk<WindowHandle> ) {

    });
    methods!(cls ( this: Gtk<WindowHandle> ) {

    });
});
