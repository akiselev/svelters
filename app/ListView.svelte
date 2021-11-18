<script lang="ts">
    import {
        createEventDispatcher,
        afterUpdate,
        onMount,
        onDestroy,
    } from "svelte";
    import { writable } from "svelte/store";
    import Item from "./Item.svelte";

    let model = new CallbackListModel((index?: number) => {
        // console.log(">>>>>>>>>>>>>>>. list model: ", index);
        if (typeof index === "number") {
            return new ListItemValue({
                text: `Hello world #${index}`,
                index,
            });
        }
        return 500;
    });

    type Event = { details: [GtkSignalListItemFactory, GtkListItem] };
    let items: Set<any> = new Set();
    let count = 0;

    const onSetup = ({ details: [item] }: Event) => {
        let box;
        if (!item.child) {
            box = new GtkBox("horizontal");
            item.child = box;
            item.svExtra.id = count;
            items.add(item);
            items = items;
            count += 1;
        }
    };

    const onBind = ({ details: [item] }: Event) => {
        // To preserve GTK Object identity between calls
        item.svExtra.item = item.item;
        items = items;
    };

    const onUnbind = ({ details: [item] }: Event) => {
        item.svExtra.item = null;
        items = items;
    };

    const onTeardown = ({ details: [item] }: Event) => {
        item.child = null;
        item.item = null;
        items.delete(item);
        items = items;
    };

    let scrolled: any;

    onMount(() => {
        let factory = new GtkSignalListItemFactory();
        factory.addEventListener("bind", onBind);
        factory.addEventListener("unbind", onUnbind);
        factory.addEventListener("setup", onSetup);
        factory.addEventListener("teardown", onTeardown);

        let selection = new GtkNoSelection(model);
        let view = new GtkListView(selection, factory);
        view.vExpand = true;
        scrolled.child = view;
        return () => {
            scrolled.child = null;
            factory.removeEventListener("bind", onBind);
            factory.removeEventListener("unbind", onUnbind);
            factory.removeEventListener("setup", onSetup);
            factory.removeEventListener("teardown", onTeardown);
        };
    });
</script>

<scrolled maxContentHeight={600} vexpand={true} bind:this={scrolled} />

{#each [...items.values()] as item (item.svExtra.id)}
    <listViewProxy child={item.child}>
        <slot item={item.svExtra.item.value} />
    </listViewProxy>
{/each}
