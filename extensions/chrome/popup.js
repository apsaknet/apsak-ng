// We disable caching during development so that we always view the latest version.
// if ('serviceWorker' in navigator && window.location.hash !== "#dev") {
//     window.addEventListener('load', function () {
//         navigator.serviceWorker.register('./pwa.js');
//     });
// }

// import init from '/apsak-egui-57c7a8dd13e092be.js';
// init('/apsak-egui-57c7a8dd13e092be_bg.wasm');

// -----------

// document.querySelector("#btn").addEventListener("click", ()=>{
//     chrome.runtime.sendMessage({
//         target: "offscreen",
//         data: "message from popup"
//     }, (msg)=>{
//         if (msg){
//             alert("msg:"+msg);
//         }
//     })
// })

// chrome.runtime.onMessage.addListener((message, sender, reply)=>{
//     console.log("popup:", message)
//     let {data} = message;
//     if (data?.counter){
//         document.querySelector("#counter").textContent = `counter: ${data.counter}`;
//     }
// })

// import init from '/apsak-ng.js';
// let apsak_ng = init('/apsak-egui_bg.wasm');
import init from '/apsak-ng.js';
(async () => {
    let apsak_ng = await init('/apsak-ng_bg.wasm');

    // const wasm = await apsak.default('./apsak-wallet/apsak-wallet_bg.wasm');
    await apsak_ng.apsak_ng_main();
})();


// wasm_bindgen('/apsak-ng_bg.wasm');