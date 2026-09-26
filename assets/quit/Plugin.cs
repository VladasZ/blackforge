using System;
using System.Runtime.InteropServices;
using BepInEx;

namespace Blackforge
{
    // Keeps Cmd+Q on macOS from closing the game. Cmd+Q is only the key of
    // the Quit item in the app menu, so the plugin clears that key and macOS
    // never starts a quit. Refusing the quit in Application.wantsToQuit is
    // not enough: Unity still sends OnApplicationQuit, Valheim shuts its
    // terrain builder down and the game stays open half dead. Quit in the
    // Dock and the Quit button of the game still close it. The app writes
    // this plugin only on macOS.
    [BepInPlugin("xyz.vladas.blackforge.quit", "Blackforge Quit", "1.1.0")]
    public class QuitPlugin : BaseUnityPlugin
    {
        private const string ObjC = "/usr/lib/libobjc.dylib";

        // The menu bar may not exist yet in the first frames.
        private const int MaxFrames = 600;

        [DllImport(ObjC)]
        private static extern IntPtr objc_getClass(string name);

        [DllImport(ObjC)]
        private static extern IntPtr sel_registerName(string name);

        [DllImport(ObjC, EntryPoint = "objc_msgSend")]
        private static extern IntPtr Send(IntPtr receiver, IntPtr selector);

        [DllImport(ObjC, EntryPoint = "objc_msgSend")]
        private static extern long SendLong(IntPtr receiver, IntPtr selector);

        [DllImport(ObjC, EntryPoint = "objc_msgSend")]
        private static extern IntPtr SendIndex(IntPtr receiver, IntPtr selector, long index);

        [DllImport(ObjC, EntryPoint = "objc_msgSend")]
        private static extern IntPtr SendText(IntPtr receiver, IntPtr selector, string text);

        [DllImport(ObjC, EntryPoint = "objc_msgSend")]
        private static extern void SendObject(IntPtr receiver, IntPtr selector, IntPtr argument);

        private int frames;

        private void Update()
        {
            frames++;
            bool? cleared;
            try
            {
                cleared = ClearQuitKey();
            }
            catch (Exception error)
            {
                Logger.LogError($"Cmd+Q could not be turned off: {error}");
                enabled = false;
                return;
            }
            if (cleared == true)
            {
                Logger.LogInfo("Cmd+Q stays in the game");
                enabled = false;
            }
            else if (frames >= MaxFrames)
            {
                Logger.LogError(cleared == null
                    ? "Cmd+Q could not be turned off: no menu bar"
                    : "Cmd+Q could not be turned off: no Quit item in the menu bar");
                enabled = false;
            }
        }

        // Null while there is no menu bar yet, false when it has no Quit item.
        private static bool? ClearQuitKey()
        {
            IntPtr app = Send(objc_getClass("NSApplication"), sel_registerName("sharedApplication"));
            IntPtr menu = app == IntPtr.Zero ? IntPtr.Zero : Send(app, sel_registerName("mainMenu"));
            if (menu == IntPtr.Zero)
            {
                return null;
            }
            IntPtr terminate = sel_registerName("terminate:");
            IntPtr empty = SendText(objc_getClass("NSString"), sel_registerName("stringWithUTF8String:"), "");
            bool found = false;
            foreach (IntPtr top in Items(menu))
            {
                IntPtr submenu = Send(top, sel_registerName("submenu"));
                if (submenu == IntPtr.Zero)
                {
                    continue;
                }
                foreach (IntPtr item in Items(submenu))
                {
                    if (Send(item, sel_registerName("action")) == terminate)
                    {
                        SendObject(item, sel_registerName("setKeyEquivalent:"), empty);
                        found = true;
                    }
                }
            }
            return found;
        }

        private static IntPtr[] Items(IntPtr menu)
        {
            long count = SendLong(menu, sel_registerName("numberOfItems"));
            IntPtr[] items = new IntPtr[count];
            for (long index = 0; index < count; index++)
            {
                items[index] = SendIndex(menu, sel_registerName("itemAtIndex:"), index);
            }
            return items;
        }
    }
}
