const __sycamore_pending=new Set();
window.__sycamore_stream_complete=false;
function __sycamore_suspense(key) {
    __sycamore_pending.add(key);
    document.currentScript.remove();
    let changed;
    do {
        changed=false;
        for (const k of __sycamore_pending) {
            const s=document.querySelector(`suspense-start[data-key="${k}"]`);
            const e=document.querySelector(`suspense-end[data-key="${k}"]`);
            if (!s || !e) continue;
            const t=document.getElementById(`sycamore-suspense-${k}`);
            while (s.nextSibling !== e) s.nextSibling.remove();
            s.before(t.content);
            s.remove(); e.remove(); t.remove();
            __sycamore_pending.delete(k);
            changed=true;
        }
    } while (changed);
}
function __sycamore_finish_stream() {
    window.__sycamore_stream_complete=__sycamore_pending.size===0;
    document.currentScript.remove();
}
document.currentScript.remove();
