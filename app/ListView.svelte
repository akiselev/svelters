<script lang="ts">
    import { createEventDispatcher, afterUpdate } from "svelte";
    import Item from "./Item.svelte";

    let model = new JsListModel((index?: number) => {
        console.log("model: ", index);
        if (typeof index === "number") {
            return {
                text: `Hello world #${index}`,
                index,
            };
        }
        return 100;
    });

    type Event = { details: [SignalListItemFactory, ListItem] };

    const onSetup = ({ details: [factory, item] }: Event) => {
        console.log(`onSetup #${item.position}`, model);
        if (!item.child) {
            item.child = new HorizontalBox();
        }
        const comp = new Item({
            target: item.child,
            props: { data: null },
        });
        item.item.value.component = comp;
    };

    const onBind = ({ details: [factory, item] }: Event) => {
        console.log(`onBind #${item.position}`, model);
        item.item.value.component.$set({ data: item.item.value });
    };

    const onUnbind = ({ details: [factory, item] }: Event) => {
        console.log(`onUnbind #${item.position}`, model);
        item.item.value.component.$set({ data: null });
    };

    const onTeardown = ({ details: [factory, item] }: Event) => {
        console.log(`onTeardown #${item.position}`, model);
        if (item.item.value.component) {
            item.item.value.component.destroy();
        }
        item.child = null;
    };

    const dispatch = createEventDispatcher();
</script>

<listView vExpand={true}>
    <factory:signalListItemFactory
        on:bind={onBind}
        on:unbind={onUnbind}
        on:setup={onSetup}
        on:teardown={onTeardown}
    />
</listView>

<style>
    .btn {
        background-color: black;
    }
</style>
