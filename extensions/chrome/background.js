import {apiBuilder} from "./api.js";

import init from '/apsak-ng.js';
(async () => {

    function initPageScript(tabId, args){

        console.log("*** initPageScript", tabId, args);
        console.log("*** location", location);
        // return;

        chrome.scripting.executeScript({
            args: args||[],
            target: {tabId},
            world: "MAIN",
            func: apiBuilder
        });
    }

    //TODO: move to rust
    async function openPopup(){
        if(chrome.action?.openPopup){
            chrome.action.openPopup();
        }else{
            let win = await chrome.windows.getCurrent();
            let width = 400;
            let left = Math.max(0, win.left + win.width - width);
            chrome.windows.create({url:"popup.html", focused:true, left, width, height:600, type:"panel"})
        }
    }

    globalThis.initPageScript = initPageScript;
    globalThis.openPopup = openPopup;

    let apsak_ng = await init('/apsak-ng_bg.wasm');

    // console.log("init", init);
    // console.log("apsak_ng", apsak_ng);

    await apsak_ng.apsak_ng_background();
})();
