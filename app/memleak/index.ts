import App from "./App.svelte";

window.app = new App({
    target: document.getElementById("app"),
    props: {
        destroy: () => {
            window.app.$destroy();
            window.app = null;
            let node = document.getElementById("app");
            node.parentNode.removeChild(node);
        },
    },
});
