using System;
using System.Threading;
using System.Windows.Forms;

namespace CopyPaste.UiaCanary
{
    public sealed class NativeControl : NativeWindow
    {
        public void Create(string className, int style, IntPtr parent, int x)
        {
            CreateParams parameters = new CreateParams();
            parameters.ClassName = className;
            parameters.Caption = String.Empty;
            parameters.Style = style;
            parameters.Parent = parent;
            parameters.X = x;
            parameters.Y = 4;
            parameters.Width = 40;
            parameters.Height = 22;
            CreateHandle(parameters);
        }

        public void Close()
        {
            if (Handle != IntPtr.Zero)
                DestroyHandle();
        }
    }

    public sealed class Session : IDisposable
    {
        private const int ChildVisibleBorder = 0x50800000;
        private const int PasswordEditStyle = ChildVisibleBorder | 0x0020;
        private const int WaitMilliseconds = 5000;
        private readonly ManualResetEvent ready = new ManualResetEvent(false);
        private volatile bool stopRequested;
        private bool disposed;
        private Thread thread;
        private Form form;
        private Exception failure;
        private NativeControl passwordEdit;
        private NativeControl button;
        private NativeControl staticText;

        public IntPtr PasswordEditHandle { get; private set; }
        public IntPtr ButtonHandle { get; private set; }
        public IntPtr StaticHandle { get; private set; }

        public static Session Start()
        {
            Session session = new Session();
            session.StartCore();
            return session;
        }

        private void StartCore()
        {
            thread = new Thread(new ThreadStart(ThreadMain));
            thread.IsBackground = true;
            thread.SetApartmentState(ApartmentState.STA);
            thread.Start();
            DateTime deadline = DateTime.UtcNow.AddMilliseconds(WaitMilliseconds);
            if (!ready.WaitOne(RemainingMilliseconds(deadline)) || failure != null || stopRequested)
            {
                StopAndJoin(deadline);
                ready.Close();
                throw new InvalidOperationException("UIA canary fixture could not start.");
            }
        }

        private void ThreadMain()
        {
            Form localForm = null;
            try
            {
                if (stopRequested) return;
                localForm = new Form();
                form = localForm;
                if (stopRequested) return;
                localForm.Text = String.Empty;
                localForm.ShowInTaskbar = false;
                localForm.StartPosition = FormStartPosition.Manual;
                localForm.Left = -32000;
                localForm.Top = -32000;
                localForm.Width = 160;
                localForm.Height = 80;
                IntPtr parent = localForm.Handle;
                if (stopRequested) return;
                passwordEdit = new NativeControl();
                passwordEdit.Create("Edit", PasswordEditStyle, parent, 4);
                if (stopRequested) return;
                button = new NativeControl();
                button.Create("Button", ChildVisibleBorder, parent, 48);
                if (stopRequested) return;
                staticText = new NativeControl();
                staticText.Create("Static", ChildVisibleBorder, parent, 92);
                if (stopRequested) return;
                PasswordEditHandle = passwordEdit.Handle;
                ButtonHandle = button.Handle;
                StaticHandle = staticText.Handle;
                localForm.Shown += delegate(object sender, EventArgs args)
                {
                    if (stopRequested)
                    {
                        localForm.Close();
                        return;
                    }
                    ready.Set();
                };
                Application.Run(localForm);
            }
            catch (Exception error)
            {
                failure = error;
            }
            finally
            {
                CloseControl(passwordEdit);
                CloseControl(button);
                CloseForm(localForm);
                form = null;
                ready.Set();
            }
        }

        private static void CloseControl(NativeControl control)
        {
            if (control == null) return;
            try { control.Close(); }
            catch (Exception) { }
        }

        private static void CloseForm(Form localForm)
        {
            if (localForm == null) return;
            try { localForm.Dispose(); }
            catch (Exception) { }
        }

        private void RequestStop()
        {
            stopRequested = true;
            Form current = form;
            if (current != null && current.IsHandleCreated)
            {
                try { current.BeginInvoke(new MethodInvoker(current.Close)); }
                catch (Exception) { }
            }
        }

        private static int RemainingMilliseconds(DateTime deadline)
        {
            double remaining = (deadline - DateTime.UtcNow).TotalMilliseconds;
            return remaining > 0 ? (int)Math.Ceiling(remaining) : 0;
        }

        private void StopAndJoin(DateTime deadline)
        {
            RequestStop();
            if (thread != null && thread.IsAlive && !thread.Join(RemainingMilliseconds(deadline)))
                throw new InvalidOperationException("UIA canary fixture did not stop.");
        }

        public void Dispose()
        {
            if (disposed) return;
            StopAndJoin(DateTime.UtcNow.AddMilliseconds(WaitMilliseconds));
            ready.Close();
            disposed = true;
        }
    }
}
