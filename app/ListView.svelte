<script lang="ts">
    import { createEventDispatcher, afterUpdate, onMount } from "svelte";
    import { ELEMENTS } from "./dom/widgets";
    import Item from "./Item.svelte";

    let model = new JsListModel((index?: number) => {
        console.log(">>>>>>>>>>>>>>>. list model: ", index);
        if (typeof index === "number") {
            return new JsValue({
                text: `Hello world #${index}`,
                index,
            });
        }
        return 5;
    });

    type Event = { details: [SignalListItemFactory, ListItem] };

    const onSetup = ({ details: [factory, item] }: Event) => {
        console.log(`onSetup #${item.position}`, model);
        const box = ELEMENTS.hBox();
        if (!item.child) {
            item.child = box;
        }
        const comp = new Item({
            target: box,
            props: { data: null },
        });
        item.box = box;
        item.comp = comp;
    };

    const onBind = ({ details: [factory, item] }: Event) => {
        console.log(`onBind #${item.position}`, model);
        item.comp.$set({ data: item.item.value });
    };

    const onUnbind = ({ details: [factory, item] }: Event) => {
        console.log(`onUnbind #${item.position}`, model);
        item.comp.$set({ data: null });
    };

    const onTeardown = ({ details: [factory, item] }: Event) => {
        console.log(`onTeardown #${item.position}`, model);

        if (item.comp) {
            item.comp.destroy();
        }
        item.child = null;
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
        listView.$.child = view;
        // selection.model = model;
        // listView.factory = factory;
        // listView.model = selection;
        // model.itemsChanged(0, 0, 100);
        // selection.$.itemsChanged(0, 0, 100);
        return () => {
            listView.$.child = null;
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
