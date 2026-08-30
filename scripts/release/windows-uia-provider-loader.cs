using System.Reflection;
using System.Runtime.CompilerServices;
using System.Windows.Automation;

namespace CopyPaste.UiaProviderLoader
{
    public static class Client
    {
        [MethodImpl(MethodImplOptions.NoInlining)]
        public static void Register(AssemblyName providerAssembly)
        {
            ClientSettings.RegisterClientSideProviderAssembly(providerAssembly);
        }
    }
}
