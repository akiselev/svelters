<script lang="ts">
    import {
        createEventDispatcher,
        afterUpdate,
        onMount,
        onDestroy,
    } from "svelte";
    import Item from "./Item.svelte";

    let model = new JsListModel((index?: number) => {
        // console.log(">>>>>>>>>>>>>>>. list model: ", index);
        if (typeof index === "number") {
            return new JsValue({
                text: `Hello world #${index}`,
                index,
            });
        }
        return 500;
    });

    type Event = { details: [SignalListItemFactory, ListItem] };

    const onSetup = ({ details: [factory, item] }: Event) => {
        let box;
        if (!item.child) {
            box = new HorizontalBox();
            item.child = box;
            item.svExtra.box = box;
        } else {
            box = item.child;
        }
        const comp = new Item({
            target: box,
            props: { data: null },
        });
        item.svExtra.comp = comp;
    };

    const onBind = ({ details: [factory, item] }: Event) => {
        item.svExtra.comp.$set({ data: item.value });
    };

    const onUnbind = ({ details: [factory, item] }: Event) => {
        item.svExtra.comp.$set({ data: null });
    };

    const onTeardown = ({ details: [factory, item] }: Event) => {
        if (item.svExtra.comp) {
            item.svExtra.comp.$destroy();
            item.svExtra.comp = null;
        }
        item.svExtra.box = null;
        item.child = null;
        item.item = null;
    };

    const dispatch = createEventDispatcher();

    let listView: any;
    let selection: any;

    onMount(() => {
        let factory = new SignalListItemFactory();
        factory.addEventListener("bind", onBind);
        factory.addEventListener("unbind", onUnbind);
        factory.addEventListener("setup", onSetup);
        factory.addEventListener("teardown", onTeardown);

        let selection = new NoSelection(model);
        let view = new ListView(selection, factory);
        view.vExpand = true;
        listView.child = view;
        // selection.model = model;
        // listView.factory = factory;
        // listView.model = selection;
        // model.itemsChanged(0, 0, 100);
        // selection.itemsChanged(0, 0, 100);
        return () => {
            listView.child = null;
            factory.removeEventListener("bind", onBind);
            factory.removeEventListener("unbind", onUnbind);
            factory.removeEventListener("setup", onSetup);
            factory.removeEventListener("teardown", onTeardown);
        };
    });
</script>

<scrolled maxContentHeight={600} bind:this={listView} />

<style>
    .btn {
        background-color: black;
    }
</style>
