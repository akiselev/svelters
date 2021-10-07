use crate::inner::*;
use glib::Object as GObject;
use gtk4::{gio::ListModel, prelude::*, *};
use query_interface::{interfaces, vtable_for, ObjectClone, ObjectPartialEq};
use smallbox::smallbox;

interfaces!(Gtk<Button>: Upcast<Button>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<ActionBar>: Upcast<ActionBar>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<Actionable>: Upcast<Actionable>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<AppChooserButton>: Upcast<AppChooserButton>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<AppChooserWidget>: Upcast<AppChooserWidget>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<AppChooser>: Upcast<AppChooser>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<AspectFrame>: Upcast<AspectFrame>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<Box>: Upcast<Box>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<Calendar>: Upcast<Calendar>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<CellEditable>: Upcast<CellEditable>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<CellView>: Upcast<CellView>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<CenterBox>: Upcast<CenterBox>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<CheckButton>: Upcast<CheckButton>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<ColorButton>: Upcast<ColorButton>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<ColorChooserWidget>: Upcast<ColorChooserWidget>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<ColumnView>: Upcast<ColumnView>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<ComboBox>: Upcast<ComboBox>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<DragIcon>: Upcast<DragIcon>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<DrawingArea>: Upcast<DrawingArea>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<DropDown>: Upcast<DropDown>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<EditableLabel>: Upcast<EditableLabel>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<Editable>: Upcast<Editable>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<Entry>: Upcast<Entry>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<Expander>: Upcast<Expander>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<FileChooserWidget>: Upcast<FileChooserWidget>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<Fixed>: Upcast<Fixed>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<FlowBoxChild>: Upcast<FlowBoxChild>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<FlowBox>: Upcast<FlowBox>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<FontButton>: Upcast<FontButton>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<FontChooserWidget>: Upcast<FontChooserWidget>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<Frame>: Upcast<Frame>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<GLArea>: Upcast<GLArea>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<Grid>: Upcast<Grid>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<HeaderBar>: Upcast<HeaderBar>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<IconView>: Upcast<IconView>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<Image>: Upcast<Image>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<InfoBar>: Upcast<InfoBar>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<Label>: Upcast<Label>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<LevelBar>: Upcast<LevelBar>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<ListBase>: Upcast<ListBase>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<ListBoxRow>: Upcast<ListBoxRow>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<ListBox>: Upcast<ListBox>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<MediaControls>: Upcast<MediaControls>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<MenuButton>: Upcast<MenuButton>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<Native>: Upcast<Native>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<Notebook>: Upcast<Notebook>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<Overlay>: Upcast<Overlay>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<Paned>: Upcast<Paned>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<PasswordEntry>: Upcast<PasswordEntry>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<Picture>: Upcast<Picture>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<PopoverMenuBar>: Upcast<PopoverMenuBar>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<Popover>: Upcast<Popover>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<ProgressBar>: Upcast<ProgressBar>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<Range>: Upcast<Range>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<Revealer>: Upcast<Revealer>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<Root>: Upcast<Root>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<ScaleButton>: Upcast<ScaleButton>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<Scrollbar>: Upcast<Scrollbar>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<ScrolledWindow>: Upcast<ScrolledWindow>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<SearchBar>: Upcast<SearchBar>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<SearchEntry>: Upcast<SearchEntry>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<Separator>: Upcast<Separator>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<ShortcutLabel>: Upcast<ShortcutLabel>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<ShortcutsShortcut>: Upcast<ShortcutsShortcut>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<SpinButton>: Upcast<SpinButton>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<Spinner>: Upcast<Spinner>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<StackSidebar>: Upcast<StackSidebar>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<StackSwitcher>: Upcast<StackSwitcher>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<Stack>: Upcast<Stack>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<Statusbar>: Upcast<Statusbar>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<Switch>: Upcast<Switch>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<TextView>: Upcast<TextView>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<Text>: Upcast<Text>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<TreeExpander>: Upcast<TreeExpander>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<TreeView>: Upcast<TreeView>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<Video>: Upcast<Video>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<Viewport>: Upcast<Viewport>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<WindowControls>: Upcast<WindowControls>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);
interfaces!(Gtk<WindowHandle>: Upcast<WindowHandle>, Upcast<Widget>, Upcast<GObject>, ObjectClone, ObjectPartialEq);

pub fn button(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "button", |ctx: Ctx| Ok(smallbox!(Gtk(Button::new()))));
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("Button")
        .build()
}

pub fn action_bar(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "actionBar", |ctx: Ctx| {
        Ok(smallbox!(Gtk(ActionBar::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("ActionBar")
        .build()
}

// pub fn actionable(this: TypeBuilder) -> ElementDef {
//     let this = ElementDef::new(this, "actionable", |ctx: Ctx|  Ok(smallbox!(Gtk(Actionable::new()))));
//     this.with_interface("Object")
//         .with_interface("Widget")
//         .with_interface("Actionable")
//         .build()
// }

// pub fn app_chooser_button(this: TypeBuilder) -> ElementDef {
//     let this = ElementDef::new(this, "appChooserButton", |ctx: Ctx|  Ok(smallbox!(Gtk(AppChooserButton::new()))));
//     this.with_interface("Object")
//         .with_interface("Widget")
//         .with_interface("AppChooserButton")
//         .build()
// }

// pub fn app_chooser_widget(this: TypeBuilder) -> ElementDef {
//     let this = ElementDef::new(this, "appChooserWidget", |ctx: Ctx|  Ok(smallbox!(Gtk(AppChooserWidget::new()))));
//     this.with_interface("Object")
//         .with_interface("Widget")
//         .with_interface("AppChooserWidget")
//         .build()
// }

// pub fn app_chooser(this: TypeBuilder) -> ElementDef {
//     let this = ElementDef::new(this, "appChooser", |ctx: Ctx|  Ok(smallbox!(Gtk(AppChooser::new()))));
//     this.with_interface("Object")
//         .with_interface("Widget")
//         .with_interface("AppChooser")
//         .build()
// }

// pub fn aspect_frame(this: TypeBuilder) -> ElementDef {
//     let this = ElementDef::new(this, "aspectFrame", |ctx: Ctx|  Ok(smallbox!(Gtk(AspectFrame::new()))));
//     this.with_interface("Object")
//         .with_interface("Widget")
//         .with_interface("AspectFrame")
//         .build()
// }

pub fn hbox(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "box", |ctx: Ctx| {
        Ok(smallbox!(Gtk(Box::new(Orientation::Horizontal, 0))))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("Box")
        .build()
}

pub fn vbox(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "box", |ctx: Ctx| {
        Ok(smallbox!(Gtk(Box::new(Orientation::Vertical, 0))))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("Box")
        .build()
}

pub fn calendar(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "calendar", |ctx: Ctx| {
        Ok(smallbox!(Gtk(Calendar::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("Calendar")
        .build()
}

// pub fn cell_editable(this: TypeBuilder) -> ElementDef {
//     let this = ElementDef::new(this, "cellEditable", |ctx: Ctx|  Ok(smallbox!(Gtk(CellEditable::new()))));
//     this.with_interface("Object")
//         .with_interface("Widget")
//         .with_interface("CellEditable")
//         .build()
// }

pub fn cell_view(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "cellView", |ctx: Ctx| {
        Ok(smallbox!(Gtk(CellView::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("CellView")
        .build()
}

pub fn center_box(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "centerBox", |ctx: Ctx| {
        Ok(smallbox!(Gtk(CenterBox::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("CenterBox")
        .build()
}

pub fn check_button(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "checkButton", |ctx: Ctx| {
        Ok(smallbox!(Gtk(CheckButton::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("CheckButton")
        .build()
}

pub fn color_button(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "colorButton", |ctx: Ctx| {
        Ok(smallbox!(Gtk(ColorButton::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("ColorButton")
        .build()
}

pub fn color_chooser_widget(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "colorChooserWidget", |ctx: Ctx| {
        Ok(smallbox!(Gtk(ColorChooserWidget::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("ColorChooserWidget")
        .build()
}

pub fn column_view(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "columnView", |ctx: Ctx| {
        Ok(smallbox!(Gtk(ColumnView::new::<SelectionModel>(None))))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("ColumnView")
        .build()
}

pub fn combo_box(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "comboBox", |ctx: Ctx| {
        Ok(smallbox!(Gtk(ComboBox::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("ComboBox")
        .build()
}

// pub fn drag_icon(this: TypeBuilder) -> ElementDef {
//     let this = ElementDef::new(this, "dragIcon", |ctx: Ctx|  Ok(smallbox!(Gtk(DragIcon::new()))));
//     this.with_interface("Object")
//         .with_interface("Widget")
//         .with_interface("DragIcon")
//         .build()
// }

pub fn drawing_area(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "drawingArea", |ctx: Ctx| {
        Ok(smallbox!(Gtk(DrawingArea::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("DrawingArea")
        .build()
}

// pub fn drop_down(this: TypeBuilder) -> ElementDef {
//     let this = ElementDef::new(this, "dropDown", |ctx: Ctx|  Ok(smallbox!(Gtk(DropDown::new::<ListModel, Expression>(None, None)))));
//     this.with_interface("Object")
//         .with_interface("Widget")
//         .with_interface("DropDown")
//         .build()
// }

pub fn editable_label(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "editableLabel", |ctx: Ctx| {
        Ok(smallbox!(Gtk(EditableLabel::new(""))))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("EditableLabel")
        .build()
}

// pub fn editable(this: TypeBuilder) -> ElementDef {
//     let this = ElementDef::new(this, "editable", |ctx: Ctx| {
//         Ok(smallbox!(Gtk(Editable::new())))
//     });
//     this.with_interface("Object")
//         .with_interface("Widget")
//         .with_interface("Editable")
//         .build()
// }

pub fn entry(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "entry", |ctx: Ctx| Ok(smallbox!(Gtk(Entry::new()))));
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("Entry")
        .build()
}

pub fn expander(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "expander", |ctx: Ctx| {
        Ok(smallbox!(Gtk(Expander::new(None))))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("Expander")
        .build()
}

// pub fn file_chooser_widget(this: TypeBuilder) -> ElementDef {
//     let this = ElementDef::new(this, "fileChooserWidget", |ctx: Ctx|  Ok(smallbox!(Gtk(FileChooserWidget::new()))));
//     this.with_interface("Object")
//         .with_interface("Widget")
//         .with_interface("FileChooserWidget")
//         .build()
// }

pub fn fixed(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "fixed", |ctx: Ctx| Ok(smallbox!(Gtk(Fixed::new()))));
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("Fixed")
        .build()
}

pub fn flow_box_child(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "flowBoxChild", |ctx: Ctx| {
        Ok(smallbox!(Gtk(FlowBoxChild::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("FlowBoxChild")
        .build()
}

pub fn flow_box(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "flowBox", |ctx: Ctx| {
        Ok(smallbox!(Gtk(FlowBox::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("FlowBox")
        .build()
}

pub fn font_button(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "fontButton", |ctx: Ctx| {
        Ok(smallbox!(Gtk(FontButton::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("FontButton")
        .build()
}

pub fn font_chooser_widget(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "fontChooserWidget", |ctx: Ctx| {
        Ok(smallbox!(Gtk(FontChooserWidget::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("FontChooserWidget")
        .build()
}

pub fn frame(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "frame", |ctx: Ctx| {
        Ok(smallbox!(Gtk(Frame::new(None))))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("Frame")
        .build()
}

pub fn gl_area(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "glArea", |ctx: Ctx| Ok(smallbox!(Gtk(GLArea::new()))));
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("GlArea")
        .build()
}

pub fn grid(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "grid", |ctx: Ctx| Ok(smallbox!(Gtk(Grid::new()))));
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("Grid")
        .build()
}

pub fn header_bar(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "headerBar", |ctx: Ctx| {
        Ok(smallbox!(Gtk(HeaderBar::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("HeaderBar")
        .build()
}

pub fn icon_view(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "iconView", |ctx: Ctx| {
        Ok(smallbox!(Gtk(IconView::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("IconView")
        .build()
}

pub fn image(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "image", |ctx: Ctx| Ok(smallbox!(Gtk(Image::new()))));
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("Image")
        .build()
}

pub fn info_bar(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "infoBar", |ctx: Ctx| {
        Ok(smallbox!(Gtk(InfoBar::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("InfoBar")
        .build()
}

pub fn label(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "label", |ctx: Ctx| {
        Ok(smallbox!(Gtk(Label::new(None))))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("Label")
        .build()
}

pub fn level_bar(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "levelBar", |ctx: Ctx| {
        Ok(smallbox!(Gtk(LevelBar::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("LevelBar")
        .build()
}

// pub fn list_base(this: TypeBuilder) -> ElementDef {
//     let this = ElementDef::new(this, "listBase", |ctx: Ctx|  Ok(smallbox!(Gtk(ListBase::new()))));
//     this.with_interface("Object")
//         .with_interface("Widget")
//         .with_interface("ListBase")
//         .build()
// }

pub fn list_box_row(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "listBoxRow", |ctx: Ctx| {
        Ok(smallbox!(Gtk(ListBoxRow::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("ListBoxRow")
        .build()
}

pub fn list_box(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "listBox", |ctx: Ctx| {
        Ok(smallbox!(Gtk(ListBox::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("ListBox")
        .build()
}

pub fn media_controls(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "mediaControls", |ctx: Ctx| {
        Ok(smallbox!(Gtk(MediaControls::new::<MediaStream>(None))))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("MediaControls")
        .build()
}

pub fn menu_button(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "menuButton", |ctx: Ctx| {
        Ok(smallbox!(Gtk(MenuButton::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("MenuButton")
        .build()
}

// pub fn native(this: TypeBuilder) -> ElementDef {
//     let this = ElementDef::new(this, "native", |ctx: Ctx|  Ok(smallbox!(Gtk(Native::new()))));
//     this.with_interface("Object")
//         .with_interface("Widget")
//         .with_interface("Native")
//         .build()
// }

pub fn notebook(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "notebook", |ctx: Ctx| {
        Ok(smallbox!(Gtk(Notebook::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("Notebook")
        .build()
}

pub fn overlay(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "overlay", |ctx: Ctx| {
        Ok(smallbox!(Gtk(Overlay::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("Overlay")
        .build()
}

pub fn hpaned(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "paned", |ctx: Ctx| {
        Ok(smallbox!(Gtk(Paned::new(Orientation::Horizontal))))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("Paned")
        .build()
}

pub fn vpaned(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "paned", |ctx: Ctx| {
        Ok(smallbox!(Gtk(Paned::new(Orientation::Vertical))))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("Paned")
        .build()
}

pub fn password_entry(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "passwordEntry", |ctx: Ctx| {
        Ok(smallbox!(Gtk(PasswordEntry::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("PasswordEntry")
        .build()
}

pub fn picture(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "picture", |ctx: Ctx| {
        Ok(smallbox!(Gtk(Picture::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("Picture")
        .build()
}

// pub fn popover_menu_bar(this: TypeBuilder) -> ElementDef {
//     let this = ElementDef::new(this, "popoverMenuBar", |ctx: Ctx|  Ok(smallbox!(Gtk(PopoverMenuBar::new()))));
//     this.with_interface("Object")
//         .with_interface("Widget")
//         .with_interface("PopoverMenuBar")
//         .build()
// }

pub fn popover(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "popover", |ctx: Ctx| {
        Ok(smallbox!(Gtk(Popover::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("Popover")
        .build()
}

pub fn progress_bar(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "progressBar", |ctx: Ctx| {
        Ok(smallbox!(Gtk(ProgressBar::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("ProgressBar")
        .build()
}

// pub fn range(this: TypeBuilder) -> ElementDef {
//     let this = ElementDef::new(this, "range", |ctx: Ctx|  Ok(smallbox!(Gtk(Range::new()))));
//     this.with_interface("Object")
//         .with_interface("Widget")
//         .with_interface("Range")
//         .build()
// }

pub fn revealer(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "revealer", |ctx: Ctx| {
        Ok(smallbox!(Gtk(Revealer::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("Revealer")
        .build()
}

// pub fn root(this: TypeBuilder) -> ElementDef {
//     let this = ElementDef::new(this, "root", |ctx: Ctx|  Ok(smallbox!(Gtk(Root::new()))));
//     this.with_interface("Object")
//         .with_interface("Widget")
//         .with_interface("Root")
//         .build()
// }

// pub fn scale_button(this: TypeBuilder) -> ElementDef {
//     let this = ElementDef::new(this, "scaleButton", |ctx: Ctx|  Ok(smallbox!(Gtk(ScaleButton::new()))));
//     this.with_interface("Object")
//         .with_interface("Widget")
//         .with_interface("ScaleButton")
//         .build()
// }

pub fn hscrollbar(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "hscrollbar", |ctx: Ctx| todo!());
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("Scrollbar")
        .build()
}

pub fn vscrollbar(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "vscrollbar", |ctx: Ctx| todo!());
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("Scrollbar")
        .build()
}

pub fn scrolled_window(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "scrolledWindow", |ctx: Ctx| {
        Ok(smallbox!(Gtk(ScrolledWindow::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("ScrolledWindow")
        .build()
}

pub fn search_bar(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "searchBar", |ctx: Ctx| {
        Ok(smallbox!(Gtk(SearchBar::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("SearchBar")
        .build()
}

pub fn search_entry(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "searchEntry", |ctx: Ctx| {
        Ok(smallbox!(Gtk(SearchEntry::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("SearchEntry")
        .build()
}

pub fn hseparator(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "hseparator", |ctx: Ctx| {
        Ok(smallbox!(Gtk(Separator::new(Orientation::Horizontal))))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("Separator")
        .build()
}

pub fn vseparator(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "bseparator", |ctx: Ctx| {
        Ok(smallbox!(Gtk(Separator::new(Orientation::Vertical))))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("Separator")
        .build()
}

pub fn shortcut_label(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "shortcutLabel", |ctx: Ctx| {
        Ok(smallbox!(Gtk(ShortcutLabel::new(""))))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("ShortcutLabel")
        .build()
}

// pub fn shortcuts_shortcut(this: TypeBuilder) -> ElementDef {
//     let this = ElementDef::new(this, "shortcutsShortcut", |ctx: Ctx| {
//         Ok(smallbox!(Gtk(ShortcutsShortcut::new())))
//     });
//     this.with_interface("Object")
//         .with_interface("Widget")
//         .with_interface("ShortcutsShortcut")
//         .build()
// }

// pub fn spin_button(this: TypeBuilder) -> ElementDef {
//     let this = ElementDef::new(this, "spinButton", |ctx: Ctx| {
//         Ok(smallbox!(Gtk(SpinButton::new())))
//     });
//     this.with_interface("Object")
//         .with_interface("Widget")
//         .with_interface("SpinButton")
//         .build()
// }

pub fn spinner(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "spinner", |ctx: Ctx| {
        Ok(smallbox!(Gtk(Spinner::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("Spinner")
        .build()
}

pub fn stack_sidebar(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "stackSidebar", |ctx: Ctx| {
        Ok(smallbox!(Gtk(StackSidebar::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("StackSidebar")
        .build()
}

pub fn stack_switcher(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "stackSwitcher", |ctx: Ctx| {
        Ok(smallbox!(Gtk(StackSwitcher::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("StackSwitcher")
        .build()
}

pub fn stack(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "stack", |ctx: Ctx| Ok(smallbox!(Gtk(Stack::new()))));
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("Stack")
        .build()
}

pub fn statusbar(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "statusbar", |ctx: Ctx| {
        Ok(smallbox!(Gtk(Statusbar::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("Statusbar")
        .build()
}

pub fn switch(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "switch", |ctx: Ctx| Ok(smallbox!(Gtk(Switch::new()))));
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("Switch")
        .build()
}

pub fn text_view(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "textView", |ctx: Ctx| {
        Ok(smallbox!(Gtk(TextView::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("TextView")
        .build()
}

pub fn text(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "text", |ctx: Ctx| Ok(smallbox!(Gtk(Text::new()))));
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("Text")
        .build()
}

pub fn tree_expander(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "treeExpander", |ctx: Ctx| {
        Ok(smallbox!(Gtk(TreeExpander::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("TreeExpander")
        .build()
}

pub fn tree_view(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "treeView", |ctx: Ctx| {
        Ok(smallbox!(Gtk(TreeView::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("TreeView")
        .build()
}

pub fn video(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "video", |ctx: Ctx| Ok(smallbox!(Gtk(Video::new()))));
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("Video")
        .build()
}

pub fn viewport(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "viewport", |ctx: Ctx| {
        Ok(smallbox!(Gtk(Viewport::new::<Adjustment, Adjustment>(
            None, None
        ))))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("Viewport")
        .build()
}

pub fn window_controls(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "windowControls", |ctx: Ctx| {
        Ok(smallbox!(Gtk(WindowControls::new(PackType::Start))))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("WindowControls")
        .build()
}

pub fn window_handle(this: TypeBuilder) -> ElementDef {
    let this = ElementDef::new(this, "windowHandle", |ctx: Ctx| {
        Ok(smallbox!(Gtk(WindowHandle::new())))
    });
    this.with_interface("Object")
        .with_interface("Widget")
        .with_interface("WindowHandle")
        .build()
}

pub fn button_interface(this: &mut ElementDef) {
    props!(|this: Button| {
        label: String {
            this.label().map(|val| val.to_string()).unwrap_or_else(||"".to_string());
            this.set_label(label.as_str());
        };
    });
    this.event(
        "click",
        GtkEvent::new::<dyn Upcast<Button>>(
            "Button",
            "click",
            |btn, sub| {
                tracing::info!("Inner event impl for Button::clicked");
                Ok(btn.upcast().connect_clicked(move |_| {
                    tracing::info!("Dispatching Button::clicked");
                    sub.dispatch(())
                }))
            },
            |btn, sub| Ok(btn.upcast().disconnect(sub)),
        ),
    );
}

pub fn action_bar_interface(this: &mut ElementDef) {}

pub fn actionable_interface(this: &mut ElementDef) {}

pub fn app_chooser_button_interface(this: &mut ElementDef) {}

pub fn app_chooser_widget_interface(this: &mut ElementDef) {}

pub fn app_chooser_interface(this: &mut ElementDef) {}

pub fn aspect_frame_interface(this: &mut ElementDef) {}

pub fn box_interface(this: &mut ElementDef) {}

pub fn calendar_interface(this: &mut ElementDef) {}

pub fn cell_editable_interface(this: &mut ElementDef) {}

pub fn cell_view_interface(this: &mut ElementDef) {}

pub fn center_box_interface(this: &mut ElementDef) {}

pub fn check_button_interface(this: &mut ElementDef) {}

pub fn color_button_interface(this: &mut ElementDef) {}

pub fn color_chooser_widget_interface(this: &mut ElementDef) {}

pub fn column_view_interface(this: &mut ElementDef) {}

pub fn combo_box_interface(this: &mut ElementDef) {}

pub fn drag_icon_interface(this: &mut ElementDef) {}

pub fn drawing_area_interface(this: &mut ElementDef) {}

pub fn drop_down_interface(this: &mut ElementDef) {}

pub fn editable_label_interface(this: &mut ElementDef) {}

pub fn editable_interface(this: &mut ElementDef) {}

pub fn entry_interface(this: &mut ElementDef) {}

pub fn expander_interface(this: &mut ElementDef) {}

pub fn file_chooser_widget_interface(this: &mut ElementDef) {}

pub fn fixed_interface(this: &mut ElementDef) {}

pub fn flow_box_child_interface(this: &mut ElementDef) {}

pub fn flow_box_interface(this: &mut ElementDef) {}

pub fn font_button_interface(this: &mut ElementDef) {}

pub fn font_chooser_widget_interface(this: &mut ElementDef) {}

pub fn frame_interface(this: &mut ElementDef) {}

pub fn gl_area_interface(this: &mut ElementDef) {}

pub fn grid_interface(this: &mut ElementDef) {}

pub fn header_bar_interface(this: &mut ElementDef) {}

pub fn icon_view_interface(this: &mut ElementDef) {}

pub fn image_interface(this: &mut ElementDef) {}

pub fn info_bar_interface(this: &mut ElementDef) {}

pub fn label_interface(this: &mut ElementDef) {}

pub fn level_bar_interface(this: &mut ElementDef) {}

pub fn list_base_interface(this: &mut ElementDef) {}

pub fn list_box_row_interface(this: &mut ElementDef) {}

pub fn list_box_interface(this: &mut ElementDef) {}

pub fn media_controls_interface(this: &mut ElementDef) {}

pub fn menu_button_interface(this: &mut ElementDef) {}

pub fn native_interface(this: &mut ElementDef) {}

pub fn notebook_interface(this: &mut ElementDef) {}

pub fn overlay_interface(this: &mut ElementDef) {}

pub fn paned_interface(this: &mut ElementDef) {}

pub fn password_entry_interface(this: &mut ElementDef) {}

pub fn picture_interface(this: &mut ElementDef) {}

pub fn popover_menu_bar_interface(this: &mut ElementDef) {}

pub fn popover_interface(this: &mut ElementDef) {}

pub fn progress_bar_interface(this: &mut ElementDef) {}

pub fn range_interface(this: &mut ElementDef) {}

pub fn revealer_interface(this: &mut ElementDef) {}

pub fn root_interface(this: &mut ElementDef) {}

pub fn scale_button_interface(this: &mut ElementDef) {}

pub fn scrollbar_interface(this: &mut ElementDef) {}

pub fn scrolled_window_interface(this: &mut ElementDef) {}

pub fn search_bar_interface(this: &mut ElementDef) {}

pub fn search_entry_interface(this: &mut ElementDef) {}

pub fn separator_interface(this: &mut ElementDef) {}

pub fn shortcut_label_interface(this: &mut ElementDef) {}

pub fn shortcuts_shortcut_interface(this: &mut ElementDef) {}

pub fn spin_button_interface(this: &mut ElementDef) {}

pub fn spinner_interface(this: &mut ElementDef) {}

pub fn stack_sidebar_interface(this: &mut ElementDef) {}

pub fn stack_switcher_interface(this: &mut ElementDef) {}

pub fn stack_interface(this: &mut ElementDef) {}

pub fn statusbar_interface(this: &mut ElementDef) {}

pub fn switch_interface(this: &mut ElementDef) {}

pub fn text_view_interface(this: &mut ElementDef) {}

pub fn text_interface(this: &mut ElementDef) {}

pub fn tree_expander_interface(this: &mut ElementDef) {}

pub fn tree_view_interface(this: &mut ElementDef) {}

pub fn video_interface(this: &mut ElementDef) {}

pub fn viewport_interface(this: &mut ElementDef) {}

pub fn window_controls_interface(this: &mut ElementDef) {}

pub fn window_handle_interface(this: &mut ElementDef) {}
